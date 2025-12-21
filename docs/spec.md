# Zkore Desktop Wallet Spec-Driven Development Document

## Scope and platform assumptions

* Desktop first UI: keyboard and mouse primary, large screen layouts, copy/paste workflows, multi window support for QR workflows and hardware wallet signing.
* Zcash support target: Orchard shielded pool only for shielded funds. Transparent pool supported only as a compatibility rail for receiving, and for shielding into Orchard.
* Privacy by default: the default path should avoid exposing transparent addresses or encouraging transparent spending, consistent with Zashi’s shielded storage posture. ([Zcash Community Forum][1])

## Requirements

### 1. Create and restore wallets from seed phrases

Users must be able to create a new wallet or restore an existing wallet using a seed phrase with clear guidance for backup and recovery.

Desktop UX requirements:

* Fast create: allow wallet creation without forcing backup before first receive, but keep a persistent, unavoidable reminder until backup is completed, and require backup before first spend. This matches Zashi's direction of reducing onboarding friction while still guiding users to back up. ([Zcash Community Forum][1])
* Restore guidance and progress: during restore, show clear progress states and actionable guidance, including an optional "approximate date of first transaction" helper to estimate wallet birthday height and reduce scan time (mirroring Zashi's restore improvements). ([Zcash Community Forum][1])
* Spend before sync: support a "spend before sync" mode (or an explicit design placeholder for it) for restored wallets so users can access already available funds without waiting for a full historical scan, consistent with Zashi's positioning. ([Electric Coin Company][2])

Network selection requirements:

* Users must choose between testnet and mainnet during wallet creation.
* Network selection is immutable after wallet creation. Changing networks requires creating a new wallet.
* Address prefixes differ by network:
  * Mainnet: u1 (Unified Addresses), zs (Sapling), t1/t3 (transparent)
  * Testnet: utest (Unified Addresses), ztestsapling (Sapling), tm (transparent)
* Display network indicator clearly in the UI to prevent confusion.

Acceptance criteria:

* New wallet can be created in under 1 minute.
* Restore flow communicates: scanning, estimated time, what is available now vs later.
* Backup completion is verifiable by re entering specific words.
* Network selection is explicit during creation and clearly displayed throughout the app.

### 2. Shielded Zcash transactions with optional memos (Orchard only)

Users must be able to send and receive shielded Zcash transactions using Orchard, with an optional memo when sending.

Zashi aligned privacy defaults:

* Shielded is the default for send and receive.
* Do not allow spending transparent ZEC directly. Any transparent funds must be shielded before they become spendable. This is explicitly how Zashi guides users to avoid privacy loss. ([Zcash Community Forum][1])
* Provide a one click “Shield and consolidate funds” action that moves all transparent value into Orchard and optionally consolidates small notes for usability, with a clear explanation of privacy benefits. ([Zcash Community Forum][1])

Receive address UX (critical update):

* Default receive address must be a shielded only Unified Address that does not include a transparent receiver.
* The receive screen should generate a fresh shielded address when opened (address rotation) while keeping all addresses under the same wallet balance.
* Provide a separately displayed transparent address (t address) only as a compatibility option for wallets and exchanges that cannot send to shielded receivers. Zashi moved to this model specifically to avoid unintentional disclosure from transparent receivers embedded in UAs. ([Zcash Community Forum][3])
* Desktop must make address sharing easy and safe: big QR, one click copy, show full address, and an “explain” affordance describing when to use shielded vs transparent. ([Zcash Community Forum][3])

Transaction UX:

* Optional memo on shielded sends.
* Show incoming transactions quickly (include mempool detection as a UX goal so “incoming” appears as soon as broadcast, then transitions to confirmed). ([Electric Coin Company][4])

Acceptance criteria:

* User can send Orchard transaction with optional memo.
* Transparent funds are visible but not spendable until shielded.
* Receive screen defaults to shielded only address and makes transparent address an explicit secondary option.

### 3. Keystone hardware wallet support (Zashi comparable UX)

Users must be able to use a Keystone hardware wallet for approvals and signing, with an experience comparable in simplicity to Zashi.

Zashi aligned behaviors to adopt:

* Zkore imports a Unified Full Viewing Key from Keystone so it can display balances and transactions without being able to spend. ([Zcash Community Forum][5])
* Shielded storage posture: if funds arrive via the Keystone transparent address, the user must “shield and consolidate” before spending, and shielding requires Keystone approval. ([Zcash Community Forum][6])

Desktop specific Keystone UX:

* Air gapped signing must work in both directions:

  * Display QR codes large enough for Keystone to scan.
  * Scan animated QRs from Keystone using a webcam.
  * Provide a no camera fallback using microSD import/export where feasible, because Keystone supports microSD for desktop wallet compatibility. ([Keystone Helpdesk][7])
* Provide a dedicated full screen signing window with:

  * Step by step instructions
  * A “slow QR” mode for reliability (frame rate and brightness tips)
  * Clear “verify on device” checklist: recipient address, amount, fee, and memo presence.
* Avoid leaking hardware wallet usage in shared artifacts: do not brand signing QRs with a hardware wallet logo, consistent with Zashi removing Keystone branding from QR codes for privacy. ([App Store][8])

Acceptance criteria:

* User can connect Keystone, see balances, create a send, and complete signing without plugging in a cable.
* All signing steps clearly indicate what must be verified on the hardware screen.

### 4. DEX integration for swaps and cross chain pay (NEAR Intents)

Users must be able to initiate and complete ZEC related swaps via an integrated flow that feels like Zashi’s NEAR Intents experience.

Required flows:

* Swap to ZEC (decentralized on ramp):

  * Choose source asset and amount
  * Provide refund address for the source chain
  * Review quote, fees, deadlines
  * Present a QR code to pay from an external wallet
  * Track status through completion and surface any refund outcome
    Zashi documents this exact sequence. ([Electric Coin Company][4])
* Swap from ZEC (off ramp):

  * Select target asset, destination address, and amount
  * Show estimated ZEC spent and fees
  * Execute from shielded ZEC
* Cross chain pay (optional but recommended for parity):

  * Pay flow that lets the user enter the exact output amount in the recipient’s chosen asset, while Zkore spends shielded ZEC under the hood. Zashi describes this as the “Pay” experience powered by NEAR Intents. ([Electric Coin Company][9])

State model and UX:

* Swap and pay entries must appear in Activity immediately and auto update status without requiring the user to open the detail view, matching Zashi’s recent UX refinement. ([App Store][8])
* Use a clear state machine: Draft, Awaiting deposit, Pending, Confirming, Completed, Refunded, Failed with recovery steps.
* Always show deadlines and timers where user action is time sensitive. Zashi increased swap deadlines to reduce early refunds, indicating deadline handling is a real UX and reliability concern. ([App Store][8])

Privacy and safety requirements:

* ZEC side of the intent flow must be shielded by default. Zashi has moved Swap and Pay to use shielded addresses instead of transparent ones, and Zkore should not regress privacy here. ([App Store][8])
* For any unavoidable transparent interactions:

  * Use ephemeral transparent addresses and do not reuse a static transparent address for refunds or receipts.
  * Make this an explicit requirement, not a nice to have. Zashi has called ephemeral transparent addresses for NEAR Intents a top priority. ([Zcash Community Forum][10])
* Always communicate privacy tradeoffs: what parts are shielded, what metadata may exist off chain (quotes, routing, external wallet usage), and how to reduce linkage.

Acceptance criteria:

* User completes swap to ZEC using an external wallet deposit flow.
* User completes swap from ZEC to a target asset by providing a destination address.
* Activity shows live status updates and clear recovery for failed or refunded intents.

### 5. Tor transport layer anonymization

Users must be able to opt in to Tor for network level privacy.

Updated behavior to match Zashi direction:

* Tor is opt in, marked as Beta, and can impact performance.
* Tor should be used for wallet network activity like submitting transactions, fetching transaction data, and connecting to third party APIs, with clear toggles and status. ([Zcash Community Forum][11])
* Fail closed: if Tor is enabled and fails, the wallet should not silently fall back to direct connections. It should prompt the user to disable Tor or retry. Zashi explicitly calls out this property as a benefit of integrated Tor. ([Zcash Community Forum][11])
* If Tor is not implemented in the first desktop milestone, keep the toggle present but disabled with explicit "unavailable" messaging, and do not imply protection is active.

Implementation approach:

* Use zcash_client_backend's tor feature, which is based on Arti (the Rust Tor implementation).
* Fail closed behavior is mandatory: no silent fallback to clearnet connections.
* This implementation is proven in production by Zashi 2.1.

Acceptance criteria:

* UI always indicates Tor mode: Off, Connecting, On, Error.
* No silent fallback from Tor to direct when Tor is enabled.
* Tor feature uses zcash_client_backend's tor integration.

### 6. Wallet status widget and privacy posture indicator (value at rest)

Replace the single purpose "privacy level indicator" with a Zashi style "wallet status widget" that also includes a privacy posture summary.

What it must do:

* Always visible on Home and in the Send flow as needed.
* Summarize current state with next best action:

  * Backup incomplete: prompt to back up seed
  * Restoring or syncing: show progress and what is available
  * Transparent funds present: prompt to shield, with one click shortcut
  * All funds shielded: indicate best posture
    This maps directly to Zashi's wallet status widget goals and examples. ([Zcash Community Forum][1])
* Privacy posture calculation should prioritize "spendable shielded value" and "transparent value requiring shielding", and update in real time after shielding, receiving, and swaps.

Desktop UX:

* Provide action buttons inside the widget (Shield now, Back up now, Continue restore).
* Provide details on click instead of forcing modal chains.

Acceptance criteria:

* Any time transparent funds exist, user gets a clear prompt and a direct action to shield.
* Status updates without needing page refresh or app restart.

### 7. Custom RPC server configuration

Users must be able to configure custom lightwalletd or Zaino server URLs for both mainnet and testnet.

Server configuration requirements:

* Default server: zec.rocks with regional options (na.zec.rocks, eu.zec.rocks, etc.).
* Users can add custom server URLs for advanced use cases.
* Display security warning when configuring custom servers: "Custom servers can see your IP address and transaction patterns. Only use servers you trust."
* Connection test required before saving server configuration to validate reachability and protocol compatibility.
* Per network configuration: mainnet and testnet servers are configured independently.

Desktop UX:

* Settings screen with server configuration section.
* Server selection dropdown with defaults and custom option.
* Custom server input field with validation and test connection button.
* Clear indicator of active server in settings and optionally in status bar.

Acceptance criteria:

* User can select from default regional servers.
* User can configure custom server URL with connection validation.
* Security warning is displayed before custom server is saved.
* Connection test prevents saving invalid server configurations.

## User Stories

### 1. Create and restore wallets from seed phrases

* As a new user, I want to create a wallet quickly and start receiving funds, while the app keeps reminding me to back up until I finish.
* As a new user, I want to explicitly choose between testnet and mainnet during wallet creation, with clear explanation of the difference.
* As a returning user, I want to restore my wallet using my seed phrase with guidance that reduces restore time, like picking an approximate first use date.
* As a user restoring a wallet, I want to see clear restore progress and know when funds are usable, including support for spend before sync where possible. ([Zcash Community Forum][1])

Desktop specific:

* As a desktop user, I want seed entry to support paste, word autocomplete, and full keyboard navigation.
* As a user, I want the network (testnet/mainnet) to be clearly displayed so I never confuse which network I am using.

### 2. Shielded Zcash transactions with optional memos (Orchard only)

* As a privacy focused user, I want shielded sending and receiving to be the default path.
* As a user, I want to add an optional memo to a shielded payment.
* As a user who receives transparent value, I want the wallet to clearly show it is not spendable until I shield, and give me a one click shielding action. ([Zcash Community Forum][1])
* As a recipient, I want the Receive screen to show a fresh shielded only address by default, and I want a transparent address only when I explicitly choose it for compatibility. ([Zcash Community Forum][3])

Desktop specific:

* As a user, I want one click copy for address and amount and a QR code large enough to scan from another device.

### 3. Keystone HW wallet support

* As a hardware wallet user, I want to connect Keystone so Zkore can show my balances without being able to spend.
* As a hardware wallet user, I want to sign transactions using a camera based QR exchange, with a fallback to microSD if I do not have a webcam. ([Keystone Helpdesk][7])
* As a user, I want Zkore to tell me exactly what to verify on the Keystone screen before approving.

Desktop specific:

* As a user, I want a full screen signing mode so the Keystone can scan reliably.

### 4. NEAR Intents swaps and pay

* As a user, I want to swap into ZEC from another asset by scanning a QR code in my external wallet and then see the result tracked in Zkore. ([Electric Coin Company][4])
* As a user, I want to swap out of ZEC to another asset by providing a destination address and seeing fees and expected outcomes.
* As a user, I want swap and pay statuses to update automatically without manually opening each item. ([App Store][8])
* As a privacy conscious user, I want the wallet to avoid transparent ZEC where possible during intents flows and clearly warn me about any privacy tradeoffs. ([App Store][8])

### 5. Tor transport layer anonymization

* As a privacy conscious user, I want to enable Tor in settings and see clear confirmation that my wallet traffic is routed through Tor.
* As a user, I want the wallet to fail closed if Tor fails so it does not silently downgrade privacy. ([Zcash Community Forum][11])
* As a user, I want to know that Tor is implemented using proven, production tested technology from the Zcash ecosystem.

### 6. Wallet status widget and privacy posture indicator

* As a user, I want one place that summarizes my wallet state and tells me what to do next, including shielding and backup reminders. ([Zcash Community Forum][1])
* As a user, I want the privacy posture to update immediately as my holdings change, especially after shielding and after swaps.

### 7. Custom RPC server configuration

* As an advanced user, I want to configure a custom lightwalletd or Zaino server URL for privacy or performance reasons.
* As a user, I want clear warnings about the privacy implications of using custom servers before I save my configuration.
* As a user, I want the wallet to test my custom server before saving it so I know it will work.
* As a user in a specific region, I want to choose a regional default server for better performance.

## Explicit constraints and out of scope (for now)

* No Sapling pool support in Zkore’s shielded operations.
* No transparent spending (transparent is receive only, must shield to spend), aligning with Zashi’s shielded storage behavior. ([Zcash Community Forum][1])
* No claims of Tor protection unless Tor is actually active and connected.


[1]: https://forum.zcashcommunity.com/t/they-grow-up-so-fast-zashi-2-0/51030 "They Grow Up So Fast: Zashi 2.0 - Zashi - Zcash Community Forum"
[2]: https://electriccoin.co/blog/meet-zashi-eccs-new-mobile-wallet-for-zcash/?utm_source=chatgpt.com "Meet Zashi, ECC's new mobile wallet for Zcash"
[3]: https://forum.zcashcommunity.com/t/zashi-2-0-3-changes-to-shielded-addresses/51299 "Zashi 2.0.3: Changes to Shielded Addresses - Zashi - Zcash Community Forum"
[4]: https://electriccoin.co/blog/zashi-swaps-decentralized-on-ramp-is-live/?utm_source=chatgpt.com "Zashi Swaps: Decentralized On-Ramp is Live"
[5]: https://forum.zcashcommunity.com/t/its-here-zashi-keystone-hardware-wallet-integration-for-shielded-zec/49784 "It's Here: Zashi-Keystone Hardware Wallet Integration for Shielded ZEC - Zashi - Zcash Community Forum"
[6]: https://forum.zcashcommunity.com/t/its-here-zashi-keystone-hardware-wallet-integration-for-shielded-zec/49784?page=2 "It's Here: Zashi-Keystone Hardware Wallet Integration for Shielded ZEC - Page 2 - Zashi - Zcash Community Forum"
[7]: https://keystonewallet.crisp.help/en/article/keystone-hardware-wallet-feature-highlights-t7z66y/?utm_source=chatgpt.com "Keystone Hardware Wallet Feature Highlights"
[8]: https://apps.apple.com/cz/app/zashi-zcash-wallet/id1672392439?utm_source=chatgpt.com "Zashi: Zcash Wallet - App Store - Apple"
[9]: https://electriccoin.co/blog/private-cross-chain-payments-with-zashi-crosspay/ "Private Cross-Chain Payments with Zashi CrossPay - Electric Coin Company"
[10]: https://forum.zcashcommunity.com/t/building-momentum-ecc-update/52685 "Building Momentum. ECC Update - Ecosystem Updates - Zcash Community Forum"
[11]: https://forum.zcashcommunity.com/t/zashi-2-1-enhanced-privacy-with-tor-beta/51865 "Zashi 2.1: Enhanced Privacy with Tor (Beta) - Zashi - Zcash Community Forum"
