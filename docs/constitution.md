# Zkore Desktop Constitution (Supplement)

The canonical constitution is `.specify/memory/constitution.md`.

This file preserves additional non-negotiable rules that were present in the prior long-form constitution but are not currently captured elsewhere in the repo. It intentionally avoids restating core principles.

## Definitions

- **UI**: The WebView frontend written in React + TypeScript.
- **Backend**: The Rust side, including wallet engine, networking, signing helpers, persistence, and all OS integrations.
- **Secret material**: Seed phrases, spending keys, signing keys, raw unsigned or signed payloads that can be used to spend, and any equivalent sensitive state.
- **Watch-only**: An account that can view balances and transactions but cannot spend.
- **Fail-closed**: When a privacy mode is enabled, operations must error rather than silently downgrade to a less private path.
- **Typed IPC**: A fixed, versioned contract between UI and backend with explicit request/response models.

## Scope and non-goals

### Purpose

Zkore Desktop provides a desktop-first experience for shielded Zcash usage with strong privacy defaults, a clear security boundary, and first-class air-gapped signing support.

### Non-goals (unless added via amendment)

- Custodial services
- Browser-extension support

## Networking and external dependencies

### Server interoperability

- The client MUST remain compatible with the CompactTxStreamer gRPC API used by lightwalletd-class servers.
- Server-specific extensions MUST be optional and MUST NOT break baseline compatibility.

### Backend-owned networking

- All outbound networking to chain servers, swap/pay APIs, and exchange-rate providers MUST be initiated by the backend, not the UI.

### Third-party API wrapper requirements

Any new external API integration MUST be mediated by a backend wrapper that provides:

- timeouts
- retry policy
- structured error mapping
- input validation

It MUST also:

- persist only the minimum required metadata locally
- include a privacy review note in the change record for the integration

## Event-driven UI (no polling-first)

- The primary UI experience MUST be event-driven (backend-published events), not polling-first.
- The backend MUST publish events for:
  - sync progress and phase changes
  - balance changes
  - transaction lifecycle changes
  - swap/pay lifecycle changes
  - Tor status changes

## Air-gapped signing: anti-fingerprinting

- Do not include branding, device identifiers, or unique markers in exported files or QR payloads beyond what protocols require.
- Do not store payload contents in crash reports.

## Engineering standards

### Dependency policy

- Prefer small, well-maintained dependencies.
- Pin versions for security-sensitive crates.
- Review license compatibility and update risk for every new dependency.

### Secure defaults

- Features that affect privacy or safety MUST ship behind explicit toggles when risk is uncertain.

### Performance as a feature

- Optimize for fast startup and bounded memory growth.
- Never trade away correctness, privacy, or key safety for speed.

## Testing and release discipline

### Compatibility matrix

- CI or pre-release verification MUST cover multiple environments where packaging is supported.

### Release discipline

- Releases MUST include migration notes in the changelog when on-disk formats or storage behavior changes.
- Every release MUST preserve the ability to open existing wallet data and provide a clear upgrade path for metadata storage.

## Amendment log

- **2025-12-23 — Allow user-initiated "View seed phrase" flow (Status: Ratified)**
  - **Exact text change**: Permitted mnemonic flows updated to include "View seed phrase" (manual wallet-password re-authentication); related constraints updated accordingly.
  - **Reason**: Allow users to re-check their backup without forcing restore workflows.
  - **Risk analysis**: Increases mnemonic exposure surface if an attacker can access the running UI; mitigate by requiring manual wallet-password re-authentication every time, forbidding OS keychain to satisfy re-auth, and preserving existing no-persist/no-logs/clear-from-memory constraints.
  - **Migration plan**: If existing wallets do not store the mnemonic in a displayable encrypted form, add a one-time migration path that keeps mnemonic encrypted at rest and never logs/exports it.
  - **Acceptance criteria**: Seed phrase can only be displayed after manual password entry; mnemonic is never persisted by the UI, is cleared from memory after the view flow, and does not appear in logs.
