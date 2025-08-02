# Status Quo

## Current Solutions
[Claude Research](./claude_research_wallet_extensions.md)

- Metamask Snaps
  - -Limited in plugin permissions
  - -Outdated documentation
  - -Javascript
  - -Don't work on mobile
  - -No control over UX or control flow
  - -Walled garden
  - -EVEN WITH FLASKS THEY DON'T LET YOU DISTRIBUTE DEVELOPMENT BUILDS OH MY GODS THEY LIMIT YOU TO LOCALHOST WHEN INSTALLING WHY???
  - +Mainstream
- Electrum Plugins
  - -Developer-focused (requires creating public key manually)
  - -Desktop-only
  - -Plugins are not sandboxed
  - -Bitcoin only
  - +Permissive permissions
  - +No walled garden 
- Ark Desktop
  - -Desktop-only
  - -Chain-specific
  - -No control over order flow
  - +Sandboxed
  - +No Walled garden

## Why do this?

Currently all mainstream wallets are built as monoliths. There are none which gives users optionality, allowing them to smoothly add new features, increase privacy or security, or take control over their experience.  Furthermore, the functionality which is provided by most wallets is unacceptable.  It forces users into inconvenient, insecure patterns of behavior. 

## Problems I encounted while building foxguard

1. Metamask restricts you to `fetch`, meaning other communication is locked.  I can't have metamask and the snap communicate over bluetooth or LAN because of this, thus requiring the mobile device to be internet-connected.
2. Metamask snaps don't run on mobile, so I need a separate companion app.
   1. Even if snaps did work on mobile, no way to trigger actions based on push notifications or external requests.
3. No control over execution flow.  When signing a transaction, for example, I'd love to have a "go check your phone to authorize this transaction" screen popup, but you're locked into metamask's signature pipeline so can't.
4. Snaps are terribly restrictive.  I had a demo website for foxguard, where you could install the snap and try it out yourself with an emulated version of the app in your browser.  But metamask won't let other people install snaps from websites, even on their development extension.


## General issues I have

1. Wallets are tied to their UIs.  The core of a wallet should be the management of cryptographic accounts and signed messages.  That may mean signing eth transactions when interacting with a dapp, but it may also mean operating in a CLI, or on a server in production.  Generic secret management.  Why do I need to export my private key to send transactions with it via forge?

2. Browser extensions interacting with websites are truly terrible UX.  Have you ever miss-clicked?  Seen how slow they are to load?

3. Private keys should NEVER be stored in a browser extension.

4. Wallets are designed for manual operation. There's no good way to automate transaction flows, bulk operations, or integrate wallet functionality into scripts and applications.  Why can't I schedule a transaction?

5. Automatically syncing wallet state across devices is either impossible or requires trusting third-party cloud services with sensitive data.  Phantom is pretty good at this, I'm sure others are also, but I don't trust my 5-digit phantom pin.

6. New blockchain features, signature schemes, or UX improvements require waiting for wallet vendors to implement them. Users can't extend functionality themselves.  *metamask snaps account abstraction grrrr*

7. Dapps exist outside of wallets.  Why can't I have a blockchain WeChat if I want? 