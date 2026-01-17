# Design Considerations

This document captures open questions, design trade-offs, and areas where the current architecture may need refinement.

## Open Questions
- How prescriptive should we be on security and privacy? 
  - Both should ideally be very high by default, and only reduced when the user has a rational reason to decrease them. 
- Should plugins be able to communicate directly with each other?
- How do we handle plugin dependencies and service discovery?
- Should there be quotas on CPU, memory, network requests or storage usage?
- Testing frameworks for plugin interactions
- Plugin code signing and verification
- Should permissions be plugin or host-scoped?
  - Plugin-scoped means people could build new things, but it feels rather dangerous.  If plugin-scoped then the handler plugin functionally has all permissions.


## Design Trade-offs

### Type Safety vs Flexibility
Strict typing is a requirement for me - typescript sucks, let alone pure JS or python.  Too hard to keep docs, types, and functionality all in sync (apparently - *metamask!!!*).  Best to just have one source of truth.

**Current Choice:** Strict typing in host, opaque UI messages with frontend-scoped enums
- +Prevents protocol fragmentation through canonical types
- +Maximum UI flexibility across frontends
- +Clear error handling for unsupported UI types
- -UI messages cannot be validated by host
- -Potential runtime errors in UI layer

**UI Enum Pattern:** Frontends define scoped enums like `CliUiRequest` and `WebUiRequest`. Plugins handle supported variants and return errors for unsupported UI types, enabling graceful degradation and frontend-specific optimizations.

**Alternative:** Fully typed UI contracts
- +Complete type safety end-to-end
- -Limits frontend innovation
- -Requires host updates for new UI patterns

**Why No UI Permissions?**
UI interactions (displaying information, requesting input) don't create meaningful security boundaries. A malicious plugin with UI access can't do anything more harmful than what it could already do through regular UI interactions - phishing attempts or UI spoofing are possible regardless of permission granularity. A single permission is enough.

### Frontend Trust Model
**Current Choice:** Frontends have unrestricted host access
- +Realistic security model - frontends control all user interaction
- +Simplifies architecture and permissions
- +No false sense of security through ineffective restrictions
- -Requires users to trust their chosen frontend completely
- -Malicious frontends have full system access

**Security Implication:** Since frontends present all UI to users, they could fake permission dialogs or capture sensitive input regardless of restrictions. The trust boundary is between user and frontend, not between frontend and host.

### Plugin Sandboxing

**Current Choice:** WASM sandboxing with explicit permissions
- +Strong isolation and security
- +Cross-platform compatibility
- -Performance overhead
- -Limited access to system resources

**Alternative:** Native plugins with capability-based security
- +Better performance
- +Full system access when needed
- -Platform-specific compilation
- -Harder to secure and audit

Also could use other sandboxing tools / make the plugins interpreted instead of compiled (I considered Lua).  Problem is then we give up strict types, and also being able to write plugins in golang or c or whatever is cool, even if not directly supported.

https://nullderef.com/blog/plugin-start/#actual_start

### Routing Complexity
**Current Choice:** Host-level routing with ownership keys
- +Clear responsibility model
- +Prevents plugin conflicts
- -Complex ownership management
- -May limit plugin composition patterns
  - But here we can just have plugins embed others in code (?)

**Alternative:** Plugin-level routing with discovery
- +More flexible plugin interactions
- -Harder security boundaries
- -Potential for routing conflicts

