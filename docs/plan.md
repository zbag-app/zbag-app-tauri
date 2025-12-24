## Technical Context

### Environment and tooling

* **Rust 1.92.0+** with **edition 2024** (backend) - aligned with librustzcash/Zashi
* **TypeScript 5.x** (frontend)
* **bun** for package management (frontend dependencies)
* **Tauri CLI**: Install with `bun add @tauri-apps/cli`

> **Version alignment**: We track librustzcash versions (zcash_client_backend 0.21+, zcash_primitives 0.26+) for ecosystem compatibility. Use caret constraints and commit Cargo.lock.

---

## Tech stack and architecture decisions

### Desktop app framework

* **Tauri v2** (Rust backend + WebView frontend)

  * Fits desktop-first needs: fast startup, small footprint, multi-window support (QR signing windows), native file dialogs (microSD import/export), and a clear trust boundary (keys stay in Rust).

### UI layer

* **React + TypeScript**

  * Good ecosystem for desktop UX (forms, keyboard navigation, routing, state management).
  * Works cleanly with Tauri IPC, camera access (getUserMedia), and animated QR components.

### Zcash wallet engine

* **Rust wallet engine built on librustzcash**

  * `zcash_client_backend` for sync, scanning, transaction construction, and features like **pczt** and **tor** (feature-flag driven). ([Docs.rs][1])
  * `zcash_client_sqlite` for the standard SQLite wallet database implementation (avoid inventing a new wallet DB format). ([Docs.rs][2])
  * **Shielded-by-default operations** (Orchard preferred; Sapling supported as needed; transparent-input spending forbidden; transparent recipients require explicit acknowledgement).

### Light client server connectivity

* **CompactTxStreamer gRPC** to a remote server

  * **Default server**: https://lwd.zec.pro (primary), https://zec.rocks with regional endpoints (fallback)
  * **Custom server support**: Users can configure alternative lightwalletd-compatible servers with security warnings
  * **Server configuration**: `ServerConfig` includes a `network` field (mainnet/testnet) that must match the wallet's network
  * Keep the client **server-agnostic**: "lightwalletd API compatible" first, Zaino-specific additions optional later ([GitHub][3])

### Tor

* **Opt-in Tor (Beta), fail-closed**

  * Use `zcash_client_backend`'s `tor` feature flag, which provides embedded Arti (Rust Tor client) integration for lightwalletd connections. ([Zcash Community Forum][4])
  * Route "sensitive" calls (tx submit, tx fetch, third party APIs like swaps) through Tor when enabled; never silently fall back. ([Zcash Community Forum][4])

### Keystone hardware wallet

* **Air-gapped QR signing** (camera) + **microSD file fallback**
* Use Keystone’s web packages for animated QR presentation/scanning:

  * `@keystonehq/animated-qr`
  * `@keystonehq/keystone-sdk` (where it helps with QR content handling) ([dev.keyst.one][5])
* Transaction signing format: **PCZT-based flow** (unsigned request -> signer -> signed response) using `zcash_client_backend`’s pczt feature. ([Docs.rs][1])

### Swaps and cross-chain pay

* **NEAR Intents 1Click API** integration (backend-owned networking)

  * Flow: quote -> deposit to unique address -> optional submit tx hash -> poll execution status -> complete or refund. ([docs.near-intents.org][6])
  * Use Intents Explorer API for historical tracking if needed later. ([docs.near-intents.org][7])

---

## High level architecture

```
+--------------------------------------------------------------+
|                       Zkore Desktop (Tauri)                   |
|                                                              |
|  React UI (TypeScript)                                       |
|   - Home + Status Widget                                     |
|   - Receive (rotating shielded-only UA)                      |
|   - Send (shielded-by-default, memo, shield-required)        |
|   - Activity (tx + swaps + pay, live updates)                |
|   - Keystone signing full-screen window (QR + camera)        |
|   - Settings (server, Tor, advanced)                         |
|                |                                             |
|                | Tauri IPC (typed commands + events)         |
|                v                                             |
|  Rust Backend (single trust boundary for secrets)            |
|   - WalletManager (keys, accounts, lock/unlock)              |
|   - SyncService (CompactTxStreamer, scanning, progress)      |
|   - TxService (send, shield+consolidate, fee, memos)         |
|   - AddressService (rotating UA, compat t-addr)              |
|   - KeystoneService (UFVK import, PCZT build/finalize)        |
|   - SwapService (NEAR Intents quote/deposit/status machine)  |
|   - TorManager (Off/Connecting/On/Error, fail-closed)        |
|   - EventBus (push updates to UI)                            |
|                                                              |
|  Storage                                                     |
|   - encrypted zcash_client_sqlite wallet DB                  |
|   - app metadata DB (backup flags, UI prefs, swap records)   |
+--------------------------------------------------------------+

External dependencies:
- Zaino / lightwalletd gRPC server (CompactTxStreamer)          :contentReference[oaicite:9]{index=9}
- NEAR Intents 1Click REST API                                  :contentReference[oaicite:10]{index=10}
- Optional: pricing / fiat-rate provider (behind Tor when on)
```

Key boundary: **the React UI never sees spending keys**. It only receives derived addresses, balances, transaction summaries, and signer payloads; mnemonic words may be shown/entered only in explicitly permitted, transient flows (create, backup verify, restore, view seed) and must never be persisted or logged by the UI.

---

## Core backend design

### 1. Rust workspace layout

Create a workspace to keep responsibilities clear:

* `zkore-core`

  * Domain types: wallet state, sync state, tx models, swap models
  * Serialization types for IPC (strict versioning)

* `zkore-engine`

  * Wrapper around `zcash_client_backend` + `zcash_client_sqlite` ([Docs.rs][1])
  * Shielded send (Orchard preferred; Sapling supported), transparent-receive visibility, shielding rules

* `zkore-network`

  * gRPC client wrapper (CompactTxStreamer)
  * HTTP client wrapper (NEAR Intents, rates)
  * Transport abstraction that can route via Tor when enabled

* `zkore-keystone`

  * UFVK import parsing + validation
  * PCZT create/finalize helpers
  * Payload encoding helpers for QR and file export

* `zkore-app-tauri`

  * Tauri commands/events
  * Window management (main window, signing window)

### 2. Event model (critical for desktop UX)

Use a backend event bus to keep UI live without polling:

Event categories:

* `sync.progress` (height, target height, ETA estimate, phase)
* `balance.changed` (orchard spendable, orchard pending, transparent)
* `tx.changed` (mempool -> confirmed, failed, etc)
* `swap.changed` (draft -> awaiting deposit -> pending -> completed/refunded/failed)
* `tor.status` (off/connecting/on/error)

This is how you satisfy:

* restore guidance and progress
* “Activity updates automatically”
* wallet status widget updates instantly

### 3. Local persistence

You will want two stores:

1. **Encrypted Wallet DB**: managed by `zcash_client_sqlite` ([Docs.rs][2])
   * Encrypted at rest with the wallet password (not readable without unlock)
   * Memo plaintext must not be written to disk (covered by DB encryption; avoid plaintext caches)
2. **App DB or metadata store**: for things the wallet DB should not own:

* backup completion state + timestamps
* “first run done” flags
* receive address rotation history metadata (optional)
* swap/pay records + state machine snapshots
* Tor setting + last known status
* server configuration list

SQLite is fine for metadata too; it keeps everything transactionally safe and easy to migrate.

### 4. Wallet security model (lock/unlock + per-action re-auth)

This plan assumes the security posture in `specs/001-zkore-desktop-wallet/spec.md`:

* Spend-capable secret material (mnemonic / spending capability) is encrypted at rest with a user-defined wallet password.
* Optional OS keychain “remember unlock” may auto-unlock the wallet on app launch, but **must not** satisfy per-action re-auth.
* Manual wallet-password re-authentication is required for every spending attempt (send, shield, swap-from-ZEC) and for "View seed phrase".
* Wallets default to locked on restart; prompt for password on launch unless keychain auto-unlock is enabled.

---

## Feature implementation plan (mapped to the spec)

### Feature A: Create and restore wallets from seed phrases

#### A1. Create wallet

Backend flow:

1. User selects network (mainnet or testnet) during wallet creation.
2. User chooses a wallet password (optionally enables “remember unlock on this device” via OS keychain).
3. Generate mnemonic (BIP-39) and seed.
4. Create encrypted wallet DB in network-specific directory:
   - Mainnet: `~/.zkore/wallets/mainnet/`
   - Testnet: `~/.zkore/wallets/testnet/`
5. Encrypt and store the mnemonic / spending capability at rest using the wallet password (keychain-assisted unlock optional).
6. Derive account keys for **Orchard** on the selected network.
7. Immediately allow receiving (wallet remains unlocked for the active session).
8. Set `backup_required = true` and persist.

UI flow:

* “Fast create” wizard that ends at Home quickly.
* Persistent banner/widget: “Back up your seed”.
* Send actions call backend; backend enforces `backup_required == false` before allowing spend and requires manual wallet-password re-authentication per spending attempt (OS keychain “remember unlock” must not satisfy re-auth).

Backup verification:

* UI asks user to re-enter N words (example: word 3, 11, 19).
* Backend validates and sets `backup_required = false`.

Seed phrase re-display (“View seed phrase”):

* User initiates “View seed phrase”.
* UI prompts for wallet password (manual re-authentication; OS keychain must not satisfy).
* Backend decrypts mnemonic and returns it for display; UI clears from memory after the flow completes and must never persist or log it.

Acceptance checks:

* Wallet created under a minute (engineering: keep network sync off the critical path).
* Backup completion is cryptographically verifiable (word match).

#### A2. Restore wallet with birthday helper

Key requirement: optional “approximate date of first transaction” to reduce scan time.

Implementation:

* Restore wizard collects a wallet password (optionally “remember unlock”) and initializes an encrypted wallet DB; the user-provided mnemonic is encrypted at rest.
* Maintain a **checkpoint table** mapping date ranges -> approximate Zcash block height.
* UI date picker -> backend converts to a “birthday height”.
* SyncService starts scanning from birthday height forward.

Restore progress UX:

* Expose phases:

  * “Preparing”
  * “Downloading compact blocks”
  * “Scanning”
  * “Enhancing transactions”
  * “Catching up”
* Show “Available now” vs “Still scanning”.

#### A3. Spend before sync mode

Design target: allow spending when some funds are already discovered even if historical scan is continuing, consistent with ECC’s “fund availability” direction. ([Electric Coin Company][8])

Implementation approach:

* SyncService maintains two concepts:

  * `scan_frontier_height` (how far scanning progressed)
  * `wallet_tip_height` (server tip)
* Engine computes:

  * `spendable_orchard` (notes with witnesses sufficient for spending)
  * `pending_orchard` (detected but not yet spendable due to witness maturity / missing context)
* TxService allows sending using the best available anchor/witness set once the engine reports spendable notes, even if scan is incomplete.

If full implementation is too risky early:

* Ship the UI and state plumbing first (clear placeholder), but keep the backend model ready so it can be enabled without rewriting the UI.

---

### Feature B: Shielded Zcash transactions with optional memos (Orchard preferred; Sapling supported)

#### B1. Address model: rotating shielded-only UA by default

Match the Zashi behavior:

* Default receive address is a **shielded-only Unified Address with no transparent receiver**.
* Generate a new address each time Receive opens.
* Show a separate transparent address only as a compatibility option. ([Zcash Community Forum][9])

Backend:

* `AddressService.get_fresh_shielded_ua()`

  * derive next diversifier
  * encode UA containing Orchard + Sapling receivers (no transparent receiver)
* `AddressService.get_compat_transparent_address()`

  * derive a transparent address (but do not embed it in UA)

UI:

* Receive screen opens -> requests fresh UA -> renders large QR and copy button.
* A secondary tab or disclosure shows the transparent compatibility address with a clear explanation.

#### B2. Transaction construction: shielded send + optional memo

Backend:

* `TxService.build_send(recipient_address, amount, memo_opt, fee_policy)`

  * parse recipient and select receiver (for UA: Orchard then Sapling; transparent requires explicit privacy acknowledgement)
  * create Orchard or Sapling output as selected (or transparent output when explicitly acknowledged)
  * attach memo only for shielded recipients (memos are not allowed for transparent recipients)
* `TxService.submit(tx_bytes)`

  * require manual wallet-password re-authentication (per spending attempt; OS keychain must not satisfy)
  * broadcast via server gRPC
  * create Activity entry immediately as “Pending”

UI:

* Send form includes optional memo.
* Confirmation screen includes: amount, recipient, fee, memo presence.
* Before final submit, prompt for wallet password (manual re-auth) to authorize the spend.

#### B3. Mandatory shielding and “Shield and consolidate”

Rules:

* Transparent funds can be received and shown.
* Transparent funds cannot be spent directly.
* Provide 1-click shielding that moves all transparent value into Orchard, optionally consolidating small notes.

Backend:

* `TxService.build_shield_all(consolidate: bool)`

  * gather all transparent UTXOs
  * create transaction spending to wallet’s Orchard address
  * optional: add Orchard self-spend consolidation strategy (configurable thresholds)
  * require manual wallet-password re-authentication before final submit (per spending attempt; OS keychain must not satisfy)

UI:

* Wallet status widget shows “Shield now” whenever transparent balance > 0.
* Activity shows shielding tx and updates status.

---

### Feature C: Keystone hardware wallet support

#### C1. Pairing and watch-only mode (UFVK import)

Backend:

* `KeystoneService.import_ufvk(ufvk_string)`

  * validate key format and network
  * create a watch-only account in wallet DB
* SyncService scans and displays balances/txs for this account.

This matches the “view but not spend” posture from the spec.

#### C2. Air-gapped signing with QR

Workflow:

1. User creates a send/shield in UI.
2. Backend builds a **PCZT** unsigned request (no keys leave).
3. UI opens a **full-screen signing window**.
4. Window displays animated QR frames.
5. Keystone scans and returns signed payload as animated QR.
6. UI scans via webcam and hands result to backend.
7. Backend finalizes and broadcasts.

Frontend implementation:

* Use Keystone’s animated QR packages for presenting and scanning QR sequences. ([dev.keyst.one][5])
* Full-screen window includes:

  * step list
  * “slow QR mode” (lower frame rate)
  * brightness and distance tips
  * explicit checklist: address, amount, fee, memo presence

#### C3. MicroSD fallback

* Export unsigned signing request as a file (`.pczt` or structured JSON/binary).
* Import signed file from microSD.
* Backend validates signature and finalizes.

#### C4. Privacy details

* Do not brand QRs with Keystone logos.
* Avoid embedding hardware wallet identifiers in exported files.
* Ensure any crash logs never include payload contents.

---

### Feature D: Swaps and cross-chain pay (NEAR Intents)

#### D1. Service integration choice

Integrate against **NEAR Intents 1Click API**:

* It provides the exact “quote -> deposit -> status tracking -> refund on failure” flow needed for desktop. ([docs.near-intents.org][6])

Implementation detail:

* All swap/pay networking lives in the **Rust backend** so Tor can cover it and so the UI never handles API keys directly.

#### D2. Local state machine

Persist a swap/pay record immediately when user starts:

* `Draft`
* `AwaitingDeposit`
* `Pending`
* `Confirming`
* `Completed`
* `Refunded`
* `Failed`

Map the remote status fields from 1Click into these states.

#### D3. Swap to ZEC flow (decentralized on ramp)

Backend:

1. `SwapService.request_quote(intent_request)` -> returns quote + deposit address.
2. Persist record in `AwaitingDeposit`.
3. UI shows deposit QR (external wallet pays).
4. Optionally accept deposit tx hash and submit it to speed up processing (1Click supports this as optional). ([docs.near-intents.org][6])
5. Poll status on timer and push events to UI.

UI:

* Review screen: quote, fees, deadlines.
* Deposit screen: QR + copy address + timer.
* Activity entry appears immediately and updates in-place.

#### D4. Swap from ZEC flow (off ramp)

Backend:

* Request quote for output chain/asset and destination address.
* Build and send ZEC spend from **shielded funds** by default.
* Require manual wallet-password re-authentication for the ZEC spend (per spending attempt; OS keychain must not satisfy).
* Persist and track until completion/refund.

#### D5. Cross-chain pay (recommended)

Use “exact output” style quoting when supported:

* User enters “recipient gets X of token Y”
* Wallet computes required ZEC spend and executes shielded payment.
* Require manual wallet-password re-authentication for the spend (per spending attempt; OS keychain must not satisfy).

#### D6. Privacy constraints

* Prefer shielded ZEC interactions end-to-end.
* If any transparent address is required by external constraints:

  * Generate an **ephemeral transparent address per intent**.
  * Never reuse.
  * Show explicit privacy tradeoff text.

---

### Feature E: Tor transport layer anonymization (opt-in, fail-closed)

#### E1. Behavior and UI

Settings toggle: “Tor (Beta)”
Status indicator: Off, Connecting, On, Error

Fail-closed:

* If Tor is enabled and not healthy, network actions fail with an actionable prompt (retry, disable Tor), never silent fallback. This aligns with the rationale described for Zashi’s integrated Tor. ([Zcash Community Forum][4])

#### E2. Routing policy

When Tor is ON:

* Route:

  * transaction submit
  * transaction fetch/enhancement
  * third party APIs (NEAR Intents)
  * exchange-rate APIs
    through Tor. ([Zcash Community Forum][4])

Implementation:

* TorManager exposes “tor-enabled clients” to:

  * gRPC lightwalletd connections (via `zcash_client_backend` Tor client patterns) ([Zcash Community Forum][4])
  * HTTP requests (NEAR Intents) via Tor transport
* Support onion endpoints if configured (advanced).

---

### Feature F: Wallet status widget + privacy posture

Backend computes a single `WalletStatus` object that the UI can render anywhere:

Inputs:

* `backup_required`
* `sync_state` (idle/syncing/error + progress)
* `orchard_spendable`
* `transparent_total`
* `shielding_in_progress`
* `swap_in_progress_count`

Output: status + next best action:

* Backup incomplete -> “Back up now”
* Syncing -> “Continue restore” / show progress
* Transparent funds present -> “Shield now”
* All funds shielded -> “All set”

This is directly supported by the receive-address posture and rotation behavior adopted in Zashi (shielded-only UA, transparent shown separately). ([Zcash Community Forum][9])

UI requirements:

* Always visible on Home.
* Visible in Send when it should block spending (backup incomplete, no spendable balance, needs shielding).
* Clicking opens a detail drawer with explanations, not a modal chain.

---

## Desktop-specific UX implementation details

### Multi-window design

Use Tauri windows for:

* Main window: app
* Full-screen signing window: Keystone QR display + scanning
* Optional: “QR viewer” detached window for Receive screen and Swap deposit screens

This makes camera workflows smoother and matches desktop multi-tasking patterns.

### QR strategy

* Standard QR (receive addresses, deposit addresses): simple QR generator
* Animated QR (Keystone signing): `@keystonehq/animated-qr` scanner and presenter ([dev.keyst.one][5])
* Webcam scanning: use browser getUserMedia in the signing window

### Keyboard and paste support

* Seed entry:

  * paste full phrase
  * word-by-word with autocomplete
  * full keyboard navigation

---

## Implementation roadmap (milestones)

### Milestone 1: Wallet foundation (shielded-by-default, privacy defaults)

* Tauri app scaffold + IPC
* Wallet create + receive screen:

  * rotating shielded-only UA
  * separate transparent compatibility address ([Zcash Community Forum][9])
* Sync and balances from CompactTxStreamer server (Zaino/lightwalletd)
* Activity list for transactions (confirmed only initially)

Deliverable: can receive shielded funds and see them after sync.

### Milestone 2: Send + memo + mandatory shielding

* Shielded send (Orchard preferred; Sapling supported) + optional memo (shielded recipients only)
* Transparent funds visible but unspendable
* “Shield and consolidate” action + status widget integration
* Broadcast + pending status for outgoing tx

Deliverable: can send shielded payments; can shield transparent funds.

### Milestone 3: Backup gating + restore UX

* Backup reminders, verification, spend gating
* Restore flow with birthday date -> height estimate
* Restore progress UI and state
* Spend-before-sync plumbing (at minimum: “Available now vs later” model), target full enablement if feasible ([Electric Coin Company][8])

Deliverable: restore is understandable and performant.

### Milestone 4: Keystone watch-only + signing

* UFVK import and watch-only scanning
* PCZT-based signing flow (QR + camera)
* MicroSD export/import fallback
* Full-screen signing window with slow QR mode and verification checklist

Deliverable: air-gapped spend and shield approvals work end-to-end.

### Milestone 5: NEAR Intents swaps + pay

* 1Click quote, deposit QR, status polling, refunds/failed handling ([docs.near-intents.org][6])
* Activity auto-updates for swap/pay entries
* Cross-chain pay (exact output) where supported

Deliverable: users can swap to and from ZEC and track outcomes reliably.

### Milestone 6: Tor (Beta) with fail-closed

* TorManager with Off/Connecting/On/Error
* Route tx submit/fetch and swaps via Tor
* Fail-closed enforcement and UI prompts ([Zcash Community Forum][4])

Deliverable: Tor mode behaves predictably and does not silently downgrade.

---

## Engineering guardrails (so the spec stays enforceable)

* **Backend enforces privacy rules**, not UI:

  * “no transparent spending”
  * “backup required before spend”
  * “Tor enabled means no direct fallback”
* **Typed IPC**:

  * versioned request/response models
  * no “generic json blobs” for critical commands
* **Crash and log hygiene**:

  * never log seeds, UFVKs, raw PCZT payloads, raw memos by default
* **Compatibility testing matrix**:

  * at least one Zaino server + one lightwalletd server in CI/dev
  * mainnet + testnet configs

---


[1]: https://docs.rs/zcash_client_backend "zcash_client_backend - Rust"
[2]: https://docs.rs/crate/zcash_client_sqlite/latest "zcash_client_sqlite 0.19.1 - Docs.rs"
[3]: https://github.com/zingolabs/zaino?utm_source=chatgpt.com "zingolabs/zaino: Zaino is an indexer for the Zcash ..."
[4]: https://forum.zcashcommunity.com/t/zashi-2-1-enhanced-privacy-with-tor-beta/51865 "Zashi 2.1: Enhanced Privacy with Tor (Beta) - Zashi - Zcash Community Forum"
[5]: https://dev.keyst.one/docs/integration-guide-basics/install-the-sdk "Install the keystone SDK | Keystone Developer Portal"
[6]: https://docs.near-intents.org/near-intents/integration/distribution-channels/1click-api?utm_source=chatgpt.com "1Click API"
[7]: https://docs.near-intents.org/near-intents/integration/distribution-channels/intents-explorer-api?utm_source=chatgpt.com "Intents Explorer API"
[8]: https://electriccoin.co/blog/new-release-5-5-0/?utm_source=chatgpt.com "New Release 5.5.0"
[9]: https://forum.zcashcommunity.com/t/zashi-2-0-3-changes-to-shielded-addresses/51299 "Zashi 2.0.3: Changes to Shielded Addresses - Zashi - Zcash Community Forum"
