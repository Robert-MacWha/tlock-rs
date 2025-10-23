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
- rotki
  - https://rotki.com/integrations
  - Really cool - looks like a much different approach to wallets.  It's got a whole bunch of customization already built-in, and some features I never would have thought of but seem really cool
    - Customizable event notifications
    - direct integration with exchanges
    - integration with a bunch of third-party services ie defiliama, etherscan, Opensea, CoinGecko, etc.
  - In their documentation it also seems like they support integrations with dapps, but in the app I can't find them.
    - Apparently their integrations are modules, which can be found in the settings tab.  But there's only 9 there, while in the github / website there's 50+?  Not sure what's up here.
  - Regardless.  The idea of having per-account modules is interesting.  Kinda flipped - instead of registering an account for a dapp, you register a dapp to be used by a given account.  
    - I like this as a potential idea.  Maybe for plugins, instead of them accessing the keyring namespace and signing data, they should have their own per-plugin account created where control flow is:
      - Send funds to the plugin account
      - Plugin does stuff
      - Plugin sends funds back to the main account.
    - This way if permissions are granted, they're sandboxed.  Maybe worthwhile considering.  I guess that itself could be an option - if I made the router a plugin then it could do whatever account creation / routing shenanigans it wanted.
  - I also really like their UI.  It's a little confusing, I think it could be laid out better.  But it's also very transparent.
- defisaver
  - https://defisaver.com/features/recipe-creator
  - https://help.defisaver.com/
  - Their recipe creator & apps are also *very* close to what I want.  They're closed-source, and they only concern themselves with defi, but in that regard they do incredibly.
    - All the dapps are simplified and use standard UI kits.  They don't offer theming as far as I can tell, but you 100% could.
    - They offer some forms of basic automation + the recipe book & creator, which seems like it'd let you create many useful automation.
    - Their portfolio considers positions, not just tokens.  Which is one of the features I was already excited about, so cool seeing it here in addition to rotki
    - They have the simulation mode idea, where you can perform transactions & do actions without actually committing them, then see the results.
  - Downsides
    - Simulations create a new account instead of using your current one - that seems very odd.
      - Like it'll allocate a new account with 100 eth on starting the simulation.
        - Feature claims to exist, but seems broken?  I noticed that in a few other places - either their QA is lacking or they're not developing it anymore?
          - Recipe creator takes *ages* to load (5s+) and can't use the "pull token" or "permit token" actions
    - Because it's a website, it's reliant on the host, doesn't work as well on mobile, and can be a bit slow.
    - It seems like it's entirely closed.  No custom protocols or tools, no arbitrary transactions in the recipe creator (which makes sense, you still have the wallet, but is limiting for automations), and it's closed-source (or at least the website is - the contracts can be found here: https://github.com/defisaver/defisaver-v3-contracts/tree/main/contracts/exchangeV3)
      - This is the main issue. What they have is super cool, but unless it works exactly for me I can't modify or improve it. Increased security and quality standards, but also decreased viability.
        - Although it is probably worthwhile taking a lesson from them / rotik and making a stripped-down version that's verified secure. 

## Why do this?

Currently all mainstream wallets are built as monoliths. There are none which gives users optionality, allowing them to smoothly add new features, increase privacy or security, or take control over their experience.  Furthermore, the functionality which is provided by most wallets is unacceptable.  It forces users into inconvenient, insecure patterns of behavior. 

## Problems I encounter while building foxguard

1. Metamask restricts you to `fetch`, meaning other communication is locked.  I can't have metamask and the snap communicate over bluetooth or LAN because of this, thus requiring the mobile device to be internet-connected.
2. Metamask snaps don't run on mobile, so I need a separate companion app.
   1. Even if snaps did work on mobile, no way to trigger actions based on push notifications or external requests.
3. No control over execution flow.  When signing a transaction, for example, I'd love to have a "go check your phone to authorize this transaction" screen popup, but you're locked into metamask's signature pipeline so can't.
4. Snaps are terribly restrictive.  I had a demo website for foxguard, where you could install the snap and try it out yourself with an emulated version of the app in your browser.  But metamask won't let other people install snaps from websites, even on their development extension.
5. The documentation and example snaps contradict each other in fundamental ways.  And their type system is trash, so you basically need to guess at what parameters it's giving you / expecting. I spent 15 hours on this. 

## General issues I have

1. Wallets are tied to their UIs.  The core of a wallet should be the management of cryptographic accounts and signed messages.  That may mean signing eth transactions when interacting with a dapp, but it may also mean operating in a CLI, or on a server in production.  Generic secret management.  Why do I need to export my private key to send transactions with it via forge?

2. Browser extensions interacting with websites are truly terrible UX.  Have you ever miss-clicked?  Seen how slow they are to load?

3. Private keys should NEVER be stored in a browser extension.

4. Wallets are designed for manual operation. There's no good way to automate transaction flows, bulk operations, or integrate wallet functionality into scripts and applications.  Why can't I schedule a transaction?

5. Automatically syncing wallet state across devices is either impossible or requires trusting third-party cloud services with sensitive data.  Phantom is pretty good at this, I'm sure others are also, but I don't trust my 5-digit phantom pin.

6. New blockchain features, signature schemes, or UX improvements require waiting for wallet vendors to implement them. Users can't extend functionality themselves.  *metamask snaps account abstraction grrrr*

7. Dapps exist outside of wallets.  Why can't I have a blockchain WeChat if I want? 