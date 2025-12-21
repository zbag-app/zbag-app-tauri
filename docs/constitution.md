# Zkore Desktop Project Constitution

## Preamble

This document defines the non-negotiable principles and the concrete engineering rules that govern how Zkore Desktop is built. If a feature, refactor, optimization, or integration conflicts with this constitution, it does not ship until the conflict is resolved or the constitution is amended.

## Definitions

* **UI**: The WebView frontend written in React + TypeScript.
* **Backend**: The Rust side, including wallet engine, networking, signing helpers, persistence, and all OS integrations.
* **Secret material**: Seed phrases, spending keys, signing keys, raw unsigned or signed payloads that can be used to spend, and any equivalent sensitive state.
* **Watch-only**: An account that can view balances and transactions but cannot spend.
* **Orchard funds**: Shielded funds managed by Orchard.
* **Transparent funds**: UTXOs received to transparent addresses.
* **Fail-closed**: When a privacy mode is enabled, operations must error rather than silently downgrade to a less private path.
* **Typed IPC**: A fixed, versioned contract between UI and backend with explicit request/response models.

---

## Article 1: Purpose, scope, and non-goals

### 1.1 Purpose

Zkore Desktop provides a desktop-first experience for shielded Zcash usage with strong privacy defaults, a clear security boundary, and first-class air-gapped signing support.

### 1.2 Scope

Included:

* Wallet creation and restoration from mnemonic
* Orchard-only spending and shielded receives
* Sync over CompactTxStreamer gRPC
* Activity tracking for transactions and swap/pay flows
* Air-gapped signing (QR and removable media)
* Optional anonymized networking via Tor
* Swap and cross-chain pay through a backend-owned integration

### 1.3 Non-goals

The following are out of scope unless explicitly added through amendment:

* Transparent spending
* Sapling spending
* Custodial services
* Browser-extension support
* Any design that requires the UI to handle seeds or spending keys

---

## Article 2: User promises

These are guarantees the project makes to users and must be preserved through all changes.

1. **Secrets never enter the UI.** The UI must not receive, store, log, or compute with seed phrases or spending keys.
2. **Privacy defaults are strong.** Receiving and sending should prioritize shielded behavior without requiring expert configuration.
3. **No silent privacy downgrade.** If the user enables a privacy mode, the app must not quietly route around it.
4. **State is explainable.** If funds are unavailable, a transaction is pending, or restore is incomplete, the app provides a clear reason and a next action.
5. **Failure is safe.** Crashes, errors, and restarts must not leak secrets and must not corrupt wallet state.

---

## Article 3: Trust boundary and key handling

### 3.1 Single trust boundary

* The Rust backend is the only trusted component for:

  * seed generation and storage
  * key derivation
  * transaction construction and finalization
  * network submission of transactions
  * Tor routing decisions
* The UI is untrusted for secrets and must operate only on derived, non-sensitive data.

### 3.2 Prohibited data flows

* The backend must never send to the UI:

  * mnemonic words
  * raw seeds
  * spending keys or raw key material
  * raw unsigned or signed payloads unless required for external signing flows, and only in a deliberately scoped form
* The UI must never persist:

  * any secret material
  * signing payload contents beyond what is strictly needed for the active signing session

### 3.3 Memory and log hygiene

* Secret material must be:

  * kept in memory for the shortest time possible
  * stored in types that support zeroization where feasible
* Logs must:

  * default to safe redaction
  * never contain seeds, keys, full payloads, or raw memos
  * treat memos as sensitive content by default

---

## Article 4: Privacy posture and transaction rules

### 4.1 Orchard-only spending

* The wallet must construct spends only from Orchard.
* Transparent funds may be received and displayed, but they must not be spent directly.

### 4.2 Mandatory shielding

* If transparent funds exist, the product must provide a clear, guided path to move them into shielded funds.
* Any send flow must enforce the rule that only shielded funds are used.

### 4.3 Address behavior

* The default receive address must not include a transparent receiver.
* If a transparent compatibility address is exposed, it must be:

  * clearly labeled as compatibility
  * separate from the primary receive surface

### 4.4 Privacy tradeoffs must be explicit

When an integration forces behavior that reduces privacy, the UI must show:

* what changes (example: a transparent deposit address is required)
* the scope (one-time, per intent)
* how the app limits harm (example: no reuse)

---

## Article 5: Networking and external dependencies

### 5.1 Server interoperability

* The client must remain compatible with the CompactTxStreamer gRPC API used by light client servers.
* Any server-specific extensions must be optional and must not break baseline compatibility.

### 5.2 Backend-owned networking

* All outbound networking to:

  * chain servers
  * swap/pay APIs
  * exchange-rate providers
    must be initiated by the backend, not the UI.

### 5.3 Tor mode behavior

When Tor mode is enabled:

* sensitive requests must route via Tor
* the app must fail-closed when Tor is not healthy
* the app must not silently retry outside Tor
* the user must be shown actionable errors (retry, disable, change endpoint)

### 5.4 Third-party API constraints

Any new external API integration must:

* be mediated by a backend wrapper with:

  * timeouts
  * retry policy
  * structured error mapping
  * input validation
* record only the minimum required metadata locally
* include a privacy review note in the change record

---

## Article 6: Persistence and data lifecycle

### 6.1 Two-store rule

* Wallet state is stored in the standard wallet database implementation.
* App-only state (preferences, backup flags, swap state snapshots, server lists) is stored separately.

### 6.2 Database migrations

* Every schema change must include:

  * forward migration
  * rollback strategy or compatibility plan
  * automated tests covering migration behavior

### 6.3 Backups and export

* Wallet backup status must be tracked as a durable flag.
* The app must not permit spending until backup verification criteria are satisfied, unless explicitly overridden by an approved amendment.

### 6.4 Data minimization

* Store the smallest set of information needed to:

  * render balances and activity
  * resume in-progress operations safely
* Avoid storing:

  * raw payloads
  * full memo bodies in debug logs
  * hardware wallet identifiers

---

## Article 7: Event-driven state model

### 7.1 No polling-first UI

* The primary UI experience must be event-driven.
* The backend must publish events for:

  * sync progress and phase changes
  * balance changes
  * transaction lifecycle changes
  * swap/pay lifecycle changes
  * Tor status changes

### 7.2 Event contract stability

* Event schemas must be versioned.
* Breaking changes require:

  * a documented migration path
  * staged rollout across UI and backend

---

## Article 8: Air-gapped signing and watch-only accounts

### 8.1 Watch-only posture

* Watch-only imports must not enable spending.
* The UI must clearly distinguish watch-only accounts from spend-capable accounts.

### 8.2 QR and removable media signing rules

* The app must support:

  * generating an unsigned signing request
  * ingesting a signed response
  * validation before broadcast
* Signing flows must include a human-verifiable review step:

  * recipient
  * amount
  * fee
  * memo presence indicator

### 8.3 Anti-fingerprinting rules

* Do not include branding, device identifiers, or unique markers in exported files or QR payloads beyond what the protocol requires.
* Do not store payload contents in crash reports.

---

## Article 9: UX standards for desktop

### 9.1 Desktop interaction commitments

* Keyboard navigation must work across primary flows.
* Copy and paste should be supported where safe and appropriate.
* Multi-window support must be used for flows that benefit from separation (example: signing).

### 9.2 Restore experience requirements

* Restore must expose:

  * distinct phases
  * progress measures that do not mislead
  * a clear distinction between discovered-but-not-spendable and spendable funds, if present in the engine model

### 9.3 Accessibility and clarity

* Security and privacy warnings must be:

  * readable
  * specific
  * actionable
* Avoid ambiguous language like “something went wrong” without details.

---

## Article 10: Engineering standards

### 10.1 Typed IPC only

* Every IPC command and event must use versioned, strongly typed request/response models.
* Generic untyped payloads are forbidden for security-critical actions.

### 10.2 Error handling

* No panics across IPC boundaries.
* Errors must map to:

  * a stable code
  * a user-safe message
  * optional developer detail that never includes secrets

### 10.3 Dependency policy

* Prefer small, well-maintained dependencies.
* Pin versions for security-sensitive crates.
* Review license compatibility and update risk for every new dependency.

### 10.4 Secure defaults

* Features that affect privacy or safety must ship behind explicit toggles when risk is uncertain.
* Beta features must have:

  * a clear label
  * a defined failure mode
  * a rollback path

### 10.5 Performance as a feature

* Optimize for:

  * fast startup
  * responsive UI under sync load
  * bounded memory growth
* Never trade away correctness, privacy, or key safety for speed.

---

## Article 11: Testing and quality gates

### 11.1 Minimum required tests

Every milestone must include:

* unit tests for domain logic and serialization
* integration tests for database and sync boundaries
* end-to-end tests for core flows on at least one test network configuration

### 11.2 Compatibility matrix

CI or pre-release verification must cover:

* at least two server implementations or endpoints that exercise the same API contract
* multiple environments where packaging is supported

### 11.3 Security testing expectations

* Add targeted regression tests for:

  * privacy regressions (fail-open behavior, unintended transparent usage)
  * key leakage via logs or serialization
  * malformed payload ingestion in signing flows

### 11.4 Definition of done

A change is complete only when:

* the constitution remains satisfied
* tests are added or updated appropriately
* an entry is added to the change record (ADR or RFC if architectural)

---

## Article 12: Release discipline

### 12.1 Versioning and changelog

* Releases must include a changelog that highlights:

  * privacy-impacting changes
  * security-relevant fixes
  * migration notes

### 12.2 Rollback and recovery

* Every release must preserve:

  * the ability to open existing wallet data
  * a clear upgrade path for metadata storage

### 12.3 Security updates

* Critical security fixes take priority over feature work.
* If a vulnerability touches secret handling or network privacy, a patch release path must be used.

---

## Article 13: Decision-making and documentation

### 13.1 Architecture decisions

* Significant changes require a short decision record that documents:

  * the problem
  * considered options
  * chosen option and why
  * consequences and follow-ups

### 13.2 Review policy

* Security-sensitive areas require review from at least one maintainer familiar with:

  * key management
  * transaction construction
  * networking and privacy modes
  * signing flows

### 13.3 Traceability

* Every milestone deliverable must link:

  * implementation
  * tests
  * acceptance criteria

---

## Article 14: Security reporting and incident response

* Provide a private channel for vulnerability reports.
* Acknowledge reports promptly and track remediation steps internally.
* Public disclosure timing must balance user safety with transparency.

---

## Article 15: Amendments

### 15.1 Amendment requirement

Any intentional violation of a rule in this constitution requires an amendment.

### 15.2 Amendment process

An amendment must include:

* the exact text change
* the reason for change
* risk analysis (privacy, security, user impact)
* migration plan if behavior changes
* acceptance criteria for the new rule

### 15.3 Amendment approval

Amendments must be reviewed and approved by maintainers responsible for security and architecture.

---

## Appendix A: Non-negotiable checklist

Before merging work that touches wallet, signing, networking, or persistence, confirm:

* secrets cannot reach the UI
* logs remain redacted
* transparent spending is still impossible
* Tor mode cannot silently downgrade
* IPC types are versioned and validated
* migrations are tested
* failure modes are user-explainable and safe
