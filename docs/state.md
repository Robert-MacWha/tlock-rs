## Current issues
State is incredibly easy to loose. Plugins can read and write state freely via `host_get_state` and `host_set_state`.  But this means if two plugin instances read from the state, modify it, and write it back, the second will overwrite the first.
```
Instance A: get_state() → balance = 100
Instance B: get_state() → balance = 100
Instance A: modify locally → balance = 200 → set_state()
Instance B: modify locally → balance = 300 → set_state()  // Lost update!
```

This isn't great for cache or simple data, and becomes catastrophic for more complex data structures.  My revm-provider is deeply affected by this, since:
    1. Lots of cached data is lost as various concurrently running instances overwrite each other
    2. Entire transactions and blocks can be inconsistently lost if multiple instances are running concurrently, leading to irreversibly corrupted state.

I think the best approach is to implement strict state locking, and probably key-based state.  Essentially re-create key-based mutex semantics inside the plugins, across the wasm host-guest boundary.  
- Key-based so that multiple plugins can lock different keys concurrently. If the state remained monolithic, then only one plugin could ever hold the lock at a time, which would be a huge bottleneck.
- Strict locking so that plugins can't accidentally overwrite each other's data.  This means that if a plugin wants to modify some state, it must first acquire a lock on the relevant keys, and only then can it read-modify-write them.  The lock is held until the plugin explicitly releases its it, or until the plugin instance exits/crashes (to avoid deadlocks).
- The goal is to make it impossible to accidentally lose updates, even if it means making the API more complex or inefficient.  I want safety by default.
  - It is possible to bypass the locking, as is true for any mutex system, but that's an anti-pattern and exists everywhere.

This'll involve implementing something roughly like the following inside the plugin SDK for state management:
```rust
/// Lock specific keys for exclusive access (blocks until all available)
pub fn lock<T, E>(
    transport: T,
    key: impl AsRef<str>,
) -> Result<KeyStateGuard<T, E>, RpcError>

/// RAII guard - auto-saves and unlocks on drop
pub struct KeyStateGuard<T, E> {
    transport: T,
    key: String,
    data: Vec<u8>,
    _phantom: PhantomData<E>,
}

impl<T, E> KeyStateGuard<T, E> {
    /// Get value for a locked key (deserializes, returns default if missing)
    pub fn get<V: DeserializeOwned + Default>(&self) -> V

    /// Try to get value for a locked key (returns error if not found)
    pub fn try_get<V: DeserializeOwned>(&self) -> Result<V, RpcError>

    /// Set value for a locked key
    pub fn set<V: Serialize>(&mut self, value: &V) -> Result<(), RpcError>
}

impl<T, E> Drop for KeyStateGuard {
    // Automatically saves and unlocks all keys
}
```

And then on the host-side:
```rust
pub mod host {
    /// Lock specific keys (blocks until all available), returns current values
    rpc_method!(
        host_lock_key,
        LockKey,
        String,
        Vec<u8>
    );

    /// Set keys and unlock atomically
    rpc_method!(
        host_set_key_and_unlock,
        SetKeyAndUnlock,
        (String, Vec<u8>),
        ()
    );

    /// Get key snapshots (no locking)
    rpc_method!(
        host_get_key_snapshot,
        GetKeySnapshot,
        String,
        Vec<u8>
    );
}
```

I'll also need to introduce the concept of a session ID, so that (a) locks are scoped to a specific plugin instance so other instances can't unlock them, and (b) locks are automatically released if the plugin instance crashes or exits without unlocking.

Anti-Pattern to Document

```rust

// ❌ DON'T: Mix snapshots with locked writes
let old_cache = get_key(transport, "cache");  // Snapshot at T0
// ... time passes, another instance updates cache ...
let mut guard = lock_keys(transport, ["cache"])?;
guard.set("cache", &old_cache)?;  // Overwrites newer data!

// ✅ DO: Lock first, then work with current data
let mut guard = lock_keys(transport, ["cache"])?;
let mut cache = guard.get("cache");  // Current data
// ... modify cache ...
guard.set("cache", &cache)?;
```

AI's plan:
```
Session-Based Key-Value State Locking Implementation Plan

 Problem Summary

 Current Issue: Multiple isolated WASM instances can concurrently read state, modify locally, and write back - causing lost updates.

 Example Race:
 Instance A: get_state() → balance = 100
 Instance B: get_state() → balance = 100
 Instance A: modify → balance = 200 → set_state()
 Instance B: modify → balance = 300 → set_state()  // Lost update!

 Why Simple RwLock Won't Work: Each WASM instance is completely isolated. RwLock only protects the host's HashMap, not the read-modify-write cycle across separate
 RPC calls.

 Solution: Session-Based Key-Value Locking

 Key-value state with fine-grained session-based locking:

 - RAII KeyStateGuard: Auto-saves and unlocks on drop
 - Fine-grained locking: Lock only specific keys (e.g., "cache" vs "config")
 - Session tracking: Locks tied to WASM instance lifetime
 - Automatic cleanup: Locks released on panic/timeout/exit
 - Blocking waits: No retry logic needed (uses tokio::sync::Notify)
 - Concurrent snapshots: Read-only access is lock-free
 - No recursive locking: Pass guards to nested functions (simple and explicit)

 Core Design Principles

 1. Safety by default: StateGuard makes wrong usage impossible
 2. Single system that leans toward safety: Can't accidentally lose updates
 3. Simple over complex: No recursive locking - pass guards down
 4. Session lifecycle integration: Automatic lock cleanup

 Session Tracking Purpose

 Session IDs identify which WASM instance holds which locks:
 - Automatic cleanup: Locks released when instance panics/exits/times out
 - Ownership verification: Instance A can't unlock Instance B's locks
 - Deadlock prevention: Crashed instances don't lock state forever

 Session ID = "which specific WASM instance execution holds this lock"

 Architecture

 struct StateManager {
     /// Key-value state storage: PluginId → (key → data)
     data: Mutex<HashMap<PluginId, HashMap<String, Vec<u8>>>>,

     /// Per-key lock ownership: (PluginId, key) → Option<SessionId>
     key_locks: Mutex<HashMap<(PluginId, String), Option<SessionId>>>,

     /// Async notification for blocking waiters
     key_waiters: Mutex<HashMap<(PluginId, String), Arc<Notify>>>,
 }

 Plugin API (PDK)

 File: /home/rmacwha/Documents/git/tlock-rs/crates/tlock-pdk/src/state.rs

 Primary API - Lock Single Key

 /// Lock a single key for exclusive access (blocks until available)
 pub fn lock_key<T, V, E>(transport: T, key: &str) -> Result<KeyGuard<T, V, E>, RpcError>
 where
     T: SyncTransport<E>,
     V: Serialize + DeserializeOwned + Default,
     E: Into<RpcError>,

 /// RAII guard - auto-saves and unlocks on drop
 pub struct KeyGuard<T, V, E> {
     transport: T,
     key: String,
     value: V,
     modified: bool,
     _phantom: PhantomData<E>,
 }

 impl<T, V, E> KeyGuard<T, V, E> {
     /// Get immutable reference to value
     pub fn get(&self) -> &V

     /// Get mutable reference to value (marks as modified)
     pub fn get_mut(&mut self) -> &mut V

     /// Replace value entirely
     pub fn set(&mut self, value: V)

     /// Manually save (keeps lock held)
     pub fn save(&mut self) -> Result<(), RpcError>
 }

 impl<T, V, E> Deref for KeyGuard<T, V, E> {
     type Target = V;
     fn deref(&self) -> &V { &self.value }
 }

 impl<T, V, E> DerefMut for KeyGuard<T, V, E> {
     fn deref_mut(&mut self) -> &mut V {
         self.modified = true;
         &mut self.value
     }
 }

 impl<T, V, E> Drop for KeyGuard {
     // Auto-save if modified, then unlock
 }

 Read-Only API - No Locking

 /// Read snapshot of a single key (no locking, high concurrency)
 pub fn get_key<T, V, E>(transport: T, key: &str) -> V
 where
     T: SyncTransport<E>,
     V: DeserializeOwned + Default,
     E: Into<RpcError>,

 /// Try to get key (returns error if missing or deserialization fails)
 pub fn try_get_key<T, V, E>(transport: T, key: &str) -> Result<V, RpcError>
 where
     T: SyncTransport<E>,
     V: DeserializeOwned,
     E: Into<RpcError>,

 Usage Examples

 Basic locking:
 let mut guard = lock_key::<_, Cache, _>(transport, "cache")?;

 guard.insert(addr, account);  // Deref to &mut Cache
 // or
 guard.get_mut().insert(addr, account);

 // Auto-saves and unlocks on drop

 Multiple keys (compose guards):
 let mut cache_guard = lock_key::<_, Cache, _>(transport.clone(), "cache")?;
 let mut config_guard = lock_key::<_, Config, _>(transport.clone(), "config")?;

 cache_guard.set(updated_cache);
 config_guard.set(updated_config);

 // Both auto-save and unlock on drop

 Passing to nested functions:
 fn outer(transport: Transport) -> Result<(), RpcError> {
     let mut guard = lock_key::<_, Cache, _>(transport, "cache")?;

     // Pass guard to nested function instead of re-locking
     update_cache(&mut guard)?;

     Ok(())  // Auto-unlocks here
 }

 fn update_cache(guard: &mut KeyGuard<_, Cache, _>) -> Result<(), RpcError> {
     guard.insert(addr, account);  // Direct mutation via Deref
     Ok(())
 }

 Read-only access (concurrent):
 let cache: Cache = get_key(transport, "cache");
 let balance = cache.get_balance(addr);

 Anti-Pattern to Document

 // ❌ DON'T: Mix snapshots with locked writes
 let old_cache = get_key(transport, "cache");  // Snapshot at T0
 // ... time passes, another instance updates cache ...
 let mut guard = lock_key(transport, "cache")?;
 *guard = old_cache;  // Overwrites newer data!

 // ✅ DO: Lock first, then work with current data
 let mut guard = lock_key(transport, "cache")?;
 guard.insert(addr, account);  // Work with current locked data

 Host RPC Methods

 File: /home/rmacwha/Documents/git/tlock-rs/crates/tlock-api/src/lib.rs

 pub mod host {
     /// Lock a single key (blocks until available), returns current value
     rpc_method!(host_lock_key, LockKey, String, Option<Vec<u8>>);

     /// Release lock on a single key
     rpc_method!(host_unlock_key, UnlockKey, String, ());

     /// Set key and unlock atomically
     rpc_method!(host_set_key_and_unlock, SetKeyAndUnlock, (String, Vec<u8>), ());

     /// Set key without unlocking (for intermediate saves)
     rpc_method!(host_set_key, SetKey, (String, Vec<u8>), ());

     /// Get key snapshot (no locking)
     rpc_method!(host_get_key_snapshot, GetKeySnapshot, String, Option<Vec<u8>>);
 }

 Host Implementation

 File: /home/rmacwha/Documents/git/tlock-rs/crates/host/src/host.rs

 use tokio::sync::Notify;

 pub struct Host {
     // ... existing fields ...
     state: Arc<StateManager>,  // Replace old state: Mutex<HashMap<...>>
 }

 struct StateManager {
     /// Key-value state storage
     data: Mutex<HashMap<PluginId, HashMap<String, Vec<u8>>>>,

     /// Per-key lock ownership
     key_locks: Mutex<HashMap<(PluginId, String), Option<SessionId>>>,

     /// Waiters for blocked sessions
     key_waiters: Mutex<HashMap<(PluginId, String), Arc<Notify>>>,
 }

 #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
 pub struct SessionId(pub u64);

 impl StateManager {
     fn new() -> Self {
         Self {
             data: Mutex::new(HashMap::new()),
             key_locks: Mutex::new(HashMap::new()),
             key_waiters: Mutex::new(HashMap::new()),
         }
     }

     /// Lock a single key (blocks until available)
     async fn lock_key(
         &self,
         plugin_id: PluginId,
         session_id: SessionId,
         key: String,
     ) -> Result<Option<Vec<u8>>, RpcError> {
         loop {
             let notify = {
                 let mut locks = self.key_locks.lock().unwrap();
                 let lock_key = (plugin_id, key.clone());

                 match locks.get(&lock_key) {
                     None | Some(None) => {
                         // Unlocked - acquire it
                         locks.insert(lock_key, Some(session_id));

                         // Return current value
                         let data = self.data.lock().unwrap();
                         return Ok(data
                             .get(&plugin_id)
                             .and_then(|pd| pd.get(&key))
                             .cloned());
                     }
                     Some(Some(holder)) if *holder == session_id => {
                         // Re-entrant lock attempt
                         return Err(RpcError::custom(format!(
                             "Key '{}' already locked by this session. Pass the KeyGuard to nested functions instead of re-locking.",
                             key
                         )));
                     }
                     Some(Some(_other)) => {
                         // Locked by another session - wait
                         let mut waiters = self.key_waiters.lock().unwrap();
                         waiters
                             .entry(lock_key)
                             .or_insert_with(|| Arc::new(Notify::new()))
                             .clone()
                     }
                 }
             };

             notify.notified().await;
         }
     }

     fn unlock_key(
         &self,
         plugin_id: PluginId,
         session_id: SessionId,
         key: String,
     ) -> Result<(), RpcError> {
         let mut locks = self.key_locks.lock().unwrap();
         let lock_key = (plugin_id, key.clone());

         match locks.get(&lock_key) {
             Some(Some(holder)) if *holder == session_id => {
                 locks.insert(lock_key.clone(), None);

                 // Notify one waiter
                 if let Some(notify) = self.key_waiters.lock().unwrap().get(&lock_key) {
                     notify.notify_one();
                 }

                 Ok(())
             }
             Some(Some(_other)) => {
                 Err(RpcError::custom(format!(
                     "Cannot unlock key '{}' held by another session",
                     key
                 )))
             }
             _ => {
                 Err(RpcError::custom(format!("Key '{}' is not locked", key)))
             }
         }
     }

     fn set_key(
         &self,
         plugin_id: PluginId,
         session_id: SessionId,
         key: String,
         value: Vec<u8>,
     ) -> Result<(), RpcError> {
         // Verify key is locked by this session
         let locks = self.key_locks.lock().unwrap();
         let lock_key = (plugin_id, key.clone());

         match locks.get(&lock_key) {
             Some(Some(holder)) if *holder == session_id => {
                 drop(locks);

                 // Update data
                 let mut data = self.data.lock().unwrap();
                 let plugin_data = data.entry(plugin_id).or_insert_with(HashMap::new);
                 plugin_data.insert(key, value);

                 Ok(())
             }
             _ => {
                 Err(RpcError::custom(format!(
                     "Key '{}' is not locked by this session",
                     key
                 )))
             }
         }
     }

     fn get_key_snapshot(&self, plugin_id: &PluginId, key: &str) -> Option<Vec<u8>> {
         let data = self.data.lock().unwrap();
         data.get(plugin_id)
             .and_then(|pd| pd.get(key))
             .cloned()
     }

     /// Force unlock all keys held by a session (cleanup on exit)
     fn force_unlock_session(&self, plugin_id: PluginId, session_id: SessionId) {
         let mut locks = self.key_locks.lock().unwrap();

         // Find all keys locked by this session
         let keys_to_unlock: Vec<_> = locks
             .iter()
             .filter(|(k, v)| k.0 == plugin_id && **v == Some(session_id))
             .map(|(k, _)| k.clone())
             .collect();

         // Release all locks
         for lock_key in keys_to_unlock {
             locks.insert(lock_key.clone(), None);

             // Notify waiters
             if let Some(notify) = self.key_waiters.lock().unwrap().get(&lock_key) {
                 notify.notify_waiters();
             }
         }
     }
 }

 impl Host {
     pub async fn lock_key(
         &self,
         plugin_id: &PluginId,
         session_id: SessionId,
         key: String,
     ) -> Result<Option<Vec<u8>>, RpcError> {
         self.state.lock_key(*plugin_id, session_id, key).await
     }

     pub async fn unlock_key(
         &self,
         plugin_id: &PluginId,
         session_id: SessionId,
         key: String,
     ) -> Result<(), RpcError> {
         self.state.unlock_key(*plugin_id, session_id, key)
     }

     pub async fn set_key_and_unlock(
         &self,
         plugin_id: &PluginId,
         session_id: SessionId,
         params: (String, Vec<u8>),
     ) -> Result<(), RpcError> {
         let (key, value) = params;
         self.state.set_key(*plugin_id, session_id, key.clone(), value)?;
         self.state.unlock_key(*plugin_id, session_id, key)?;
         Ok(())
     }

     pub async fn set_key(
         &self,
         plugin_id: &PluginId,
         session_id: SessionId,
         params: (String, Vec<u8>),
     ) -> Result<(), RpcError> {
         let (key, value) = params;
         self.state.set_key(*plugin_id, session_id, key, value)
     }

     pub async fn get_key_snapshot(
         &self,
         plugin_id: &PluginId,
         _session_id: SessionId,
         key: String,
     ) -> Result<Option<Vec<u8>>, RpcError> {
         Ok(self.state.get_key_snapshot(plugin_id, &key))
     }

     /// Called when session ends (normal, timeout, or panic)
     pub fn on_session_end(&self, plugin_id: PluginId, session_id: SessionId) {
         self.state.force_unlock_session(plugin_id, session_id);
     }
 }

 Session Tracking Integration

 Changes to wasmi-plugin-framework

 A. Thread Session IDs (wasmi-plugin-hdk/src/plugin.rs):
 impl AsyncTransport for Plugin {
     async fn call_async(&self, method: &str, params: Value) -> Result<RpcResponse, PluginError> {
         let (session_id, stdin, stdout, stderr) = runtime.spawn(...).await?;

         let handler = PluginCallback {
             handler: self.handler.clone(),
             plugin_id: self.id,
             session_id,  // NEW
         };

         // ... execute session ...

         // Cleanup on session end
         handler.on_session_end(plugin_id, session_id);
     }
 }

 B. Update HostHandler trait (wasmi-plugin-hdk/src/host_handler.rs):
 pub trait HostHandler: Send + Sync {
     fn handle<'a>(
         &'a self,
         plugin_id: PluginId,
         session_id: u64,  // NEW
         method: &'a str,
         params: Value,
     ) -> BoxFuture<'a, Result<Value, RpcError>>;

     fn on_session_end(&self, plugin_id: PluginId, session_id: u64) {
         // Default: no-op
     }
 }

 C. Update HostServer (wasmi-plugin-hdk/src/server.rs):
 pub struct HostServer<S> {
     router: Router<(PluginId, SessionId, S)>,  // Add SessionId
 }

 D. Update macros (tlock-hdk/src/lib.rs):
 macro_rules! impl_host_rpc {
     ($host_ty:ty, $method:ty, $host_fn:ident) => {
         pub async fn $host_fn(
             host: (PluginId, u64, Weak<$host_ty>),
             params: <$method>::Params,
         ) -> Result<...> {
             let plugin_id = host.0;
             let session_id = SessionId(host.1);
             let host = host.2.upgrade()?;

             host.$host_fn(&plugin_id, session_id, params).await
         }
     };
 }

 State Persistence

 File: /home/rmacwha/Documents/git/tlock-rs/crates/host/src/host_state.rs

 No changes needed - serialization format already supports key-value:

 #[derive(Debug, Clone, Serialize, Deserialize)]
 pub struct HostState {
     pub plugins: Vec<PluginData>,
     pub entities: Vec<(EntityId, PluginId)>,
     pub state: Vec<(PluginId, HashMap<String, Vec<u8>>)>,  // Change Vec<u8> → HashMap
 }

 Update Host::to_state() and Host::from_state() to serialize/deserialize the HashMap structure.

 Example: revm-provider Migration

 Before (single monolithic state):
 #[derive(Serialize, Deserialize)]
 struct State {
     fork_block: BlockId,
     chain_id: u64,
     fork_snapshot: ProviderSnapshot,  // Large!
     page_id: Option<PageId>,
 }

 fn set_snapshot(transport: Transport, snapshot: ProviderSnapshot) -> Result<(), RpcError> {
     let mut state: State = try_get_state(transport)?;
     state.fork_snapshot = snapshot;
     set_state(transport, &state)?;  // Serialize entire state every time
     Ok(())
 }

 After (key-value with fine-grained locking):
 // Keys: "config", "snapshot", "page_id"

 fn set_snapshot(transport: Transport, snapshot: ProviderSnapshot) -> Result<(), RpcError> {
     let mut guard = lock_key::<_, ProviderSnapshot, _>(transport, "snapshot")?;
     *guard = snapshot;
     Ok(())  // Auto-saves and unlocks
 }

 fn get_config(transport: Transport) -> Config {
     get_key(transport, "config")  // Concurrent snapshot, no lock
 }

 Custom CacheDB integration:
 impl Database for LockedCacheDb {
     fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>> {
         // self.guard is KeyGuard<Transport, Cache, Error> passed from outer scope
         let cache = &mut *self.guard;  // Deref to &mut Cache

         if let Some(account) = cache.accounts.get(&address) {
             return Ok(Some(account.info.clone()));
         }

         // Cache miss - fetch from RPC
         let account_info = self.alloy_client.get_account(address).await?;
         cache.accounts.insert(address, DbAccount::from(account_info));

         // Guard will auto-save when dropped

         Ok(Some(account_info))
     }
 }

 Files to Modify

 Critical Files

 1. /home/rmacwha/Documents/git/tlock-rs/crates/tlock-pdk/src/state.rs
   - Replace monolithic API with key-value API
   - Implement KeyStateGuard with RAII cleanup
 2. /home/rmacwha/Documents/git/tlock-rs/crates/host/src/host.rs
   - Implement StateManager with key-value storage
   - Add session-based locking logic
 3. /home/rmacwha/Documents/git/tlock-rs/crates/tlock-api/src/lib.rs
   - Add key-value RPC methods
 4. /home/rmacwha/Documents/git/wasmi-plugin-framework/crates/wasmi-plugin-hdk/src/plugin.rs
   - Thread session IDs through calls
   - Add cleanup hook
 5. /home/rmacwha/Documents/git/wasmi-plugin-framework/crates/wasmi-plugin-hdk/src/host_handler.rs
   - Add session_id parameter to trait
 6. /home/rmacwha/Documents/git/wasmi-plugin-framework/crates/wasmi-plugin-hdk/src/server.rs
   - Update router state tuple
 7. /home/rmacwha/Documents/git/tlock-rs/crates/tlock-hdk/src/lib.rs
   - Update macros to pass session_id
 8. /home/rmacwha/Documents/git/tlock-rs/crates/host/src/host_state.rs
   - Update state field type to HashMap<String, Vec<u8>>

 Plugin Updates

 9. /home/rmacwha/Documents/git/tlock-rs/plugins/revm-provider/src/main.rs
   - Migrate to key-value state
   - Use fine-grained locking

 Testing Strategy

 1. Unit Tests - StateManager mechanics:
   - Lock/unlock single key
   - Force unlock on session end
   - Blocking behavior (wait for locked key)
   - Re-entrant lock detection (error)
 2. Concurrency Tests:
   - Session A locks "cache", Session B waits, A releases, B acquires
   - Session A locks "cache", Session B locks "config" concurrently (no conflict)
   - No lost updates under concurrent modifications
   - No deadlocks (single-key locking eliminates cross-key deadlock scenarios)
 3. Panic Recovery:
   - KeyGuard dropped without explicit unlock (auto-cleanup)
   - Session timeout releases locks
   - Force unlock called on session end
 4. Integration Tests:
   - revm-provider concurrent operations
   - State persistence (to_state/from_state roundtrip)

 Success Criteria

 - ✅ No lost updates (exclusive locking)
 - ✅ Fine-grained concurrency (lock only needed keys)
 - ✅ Automatic cleanup (no deadlocks on panic/timeout)
 - ✅ Safe by default (StateGuard RAII)
 - ✅ Simple (no recursive locking, pass guards down)
 - ✅ State persistence works
```