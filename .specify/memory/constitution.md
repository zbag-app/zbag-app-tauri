<!--
	SYNC IMPACT REPORT
	==================
	Version change: 1.3.1 -> 2.0.0
	Bump rationale: MAJOR - Keystone hardware wallet support, structured logging infrastructure

	Added sections:
	  - VIII. Structured Logging

	Modified principles:
	  - I. Secrets Stay in Rust (clarify watch-only wallet seed handling)

	Modified sections:
	  - Core Principles -> I. Secrets Stay in Rust
	  - Non-Negotiable Checklist

Templates requiring updates:
  - .specify/templates/plan-template.md: Constitution Check section exists
  - .specify/templates/spec-template.md: No constitution references found
  - .specify/templates/tasks-template.md: No constitution references found

Source document: (formerly) detailed constitution (removed; this file is canonical)
	==================
-->

# Zkore Desktop Constitution

## Preamble

This document defines the non-negotiable principles governing Zkore Desktop development. If a feature, refactor, or integration conflicts with these principles, it does not ship until the conflict is resolved or this constitution is amended.

This file is the canonical constitution for Zkore Desktop.

## Core Principles

### I. Secrets Stay in Rust

The Rust backend is the single trust boundary for all secret material. The UI MUST NOT store, log, or compute with spending keys or raw seeds.

**Permitted mnemonic flows (with strict constraints):**
- Mnemonic MAY be sent to UI ONLY for: initial creation display, backup verification, restore entry, and user-initiated "View seed phrase" (manual wallet-password re-authentication)
- UI MUST NOT persist mnemonic to durable storage, MUST NOT log it, MUST clear from memory after flow completes
- Backend MUST NOT re-send mnemonic after initial creation response unless the user explicitly initiates the "View seed phrase" flow (manual wallet-password re-authentication required)
- Watch-only wallets (created from UFVK, e.g., Keystone hardware wallet) have no mnemonic; "View seed phrase" MUST return an error for watch-only wallets

**Permitted payload flows:**
- Raw unsigned/signed payloads MAY cross IPC ONLY for external signing flows (Keystone PCZT)
- Software wallet flows MUST use proposal-based pattern (tx bytes stay in backend)

**Prohibited flows:**
- Backend MUST NEVER send: raw seeds (entropy bytes), spending keys, tx bytes for software wallets
- UI MUST NEVER persist secrets, log mnemonic/seeds/payloads, or retain payloads beyond active session
- Memory containing secrets MUST use zeroization where feasible
- Logs MUST redact seeds, keys, full payloads, and raw memos by default

### II. Shielded-by-Default Privacy

All user-initiated payments MUST be funded from shielded pools (Orchard + Sapling). Transparent funds MAY be received and displayed but MUST NOT be used for payments until shielded.

**Enforceable rules:**
- Default receive address MUST NOT include a transparent receiver
- Transparent funds require explicit shielding before becoming spendable
- Any privacy downgrade MUST be explicit, scoped, and user-acknowledged (e.g., sending to transparent recipients)
- Transparent inputs MAY be spent only in explicit shielding transactions that move funds into shielded pools. Transparent inputs MUST NOT be used for user-initiated payments.

### III. Fail-Closed Safety

When a privacy or safety mode is enabled, operations MUST error rather than silently downgrade. Crashes and errors MUST NOT leak secrets or corrupt wallet state.

**Enforceable rules:**
- Tor mode enabled: MUST fail if Tor unhealthy, MUST NOT silently retry direct
- Network errors: MUST surface actionable prompts (retry, disable, change endpoint)
- All failures: MUST preserve wallet state integrity and redact secrets from crash logs
- Beta features: MUST have clear label, defined failure mode, rollback path

### IV. Typed IPC Contracts

All communication between UI and backend MUST use versioned, strongly typed request/response models. Generic untyped payloads are forbidden for security-critical actions.

**Enforceable rules:**
- Every IPC command and event MUST have versioned schema
- Breaking changes MUST include documented migration path
- Errors MUST map to stable code + user-safe message + optional developer detail (no secrets)
- No panics across IPC boundaries

### V. Test-Driven Quality

Every milestone MUST include unit tests for domain logic, integration tests for database/sync boundaries, and targeted security regression tests.

**Enforceable rules:**
- Privacy regressions (fail-open, unintended transparent usage) MUST have regression tests
- Key leakage via logs or serialization MUST have regression tests
- Malformed payload ingestion in signing flows MUST have regression tests
- CI MUST cover at least two independent lightwalletd deployments (primary + secondary) to catch server-side behavior differences and regressions

### VI. Data Minimization

Store only the minimum information needed to render balances, activity, and resume in-progress operations. Separate wallet state from app-only metadata.

**Enforceable rules:**
- Wallet state: zcash_client_sqlite wallet database
- App state: separate store for preferences, backup flags, swap records, server config
- Avoid storing raw payloads, full memo bodies in logs, hardware wallet identifiers
- Every schema change MUST include forward migration + rollback strategy + tests

### VII. Decision Traceability

Significant changes MUST be documented with problem, options considered, chosen approach, and consequences. Every release MUST include changelog with privacy/security impacts.

**Enforceable rules:**
- Architectural decisions MUST have ADR/RFC documenting reasoning
- Security-sensitive reviews MUST involve maintainer familiar with key management, tx construction, networking, signing
- Every milestone deliverable MUST link implementation, tests, and acceptance criteria
- Critical security fixes MUST use patch release path

### VIII. Structured Logging

Application logs MUST support structured, redacted, and rotated file output.

**Enforceable rules:**
- Log files MUST be written to `~/.zkore/logs/` with daily rotation (e.g., `zkore.YYYY-MM-DD.log`)
- Log retention MUST be limited (default: 7 days)
- Secrets (mnemonics, seeds, spending keys, raw payloads, full memos) MUST be redacted in all log output
- Log levels MUST be configurable via `RUST_LOG` environment variable
- File logging MUST use tracing-appender or equivalent with non-blocking writes

## Non-Negotiable Checklist

Before merging work that touches wallet, signing, networking, or persistence, confirm:

- [ ] Secrets cannot reach the UI
- [ ] Mnemonic flows follow permitted patterns (create, backup verify, restore, view seed) with no UI persistence or logging; "View seed phrase" requires manual wallet-password re-authentication
- [ ] Logs remain redacted
- [ ] Transparent funds still cannot be used for payments; transparent inputs are only permitted for explicit shielding transactions (transparent -> shielded)
- [ ] Sending to transparent recipients requires explicit privacy acknowledgement
- [ ] Tor mode cannot silently downgrade
- [ ] IPC types are versioned and validated
- [ ] Migrations are tested
- [ ] CI covers integration tests against at least two independent lightwalletd deployments (primary + secondary)
- [ ] Failure modes are user-explainable and safe

## Governance

### Amendment Process

Any intentional deviation from these principles requires an amendment. An amendment MUST include:
- Exact text change
- Reason for change
- Risk analysis (privacy, security, user impact)
- Migration plan if behavior changes
- Acceptance criteria for the new rule

Amendments MUST be reviewed and approved by maintainers responsible for security and architecture.

### Versioning

This constitution follows semantic versioning:
- **MAJOR**: Backward-incompatible governance/principle removals or redefinitions
- **MINOR**: New principle/section added or materially expanded guidance
- **PATCH**: Clarifications, wording, non-semantic refinements

### Reference Documents

- **Feature specification**: `specs/001-zkore-desktop-wallet/spec.md`
- **Implementation plan**: `specs/001-zkore-desktop-wallet/plan.md`
- **Data model**: `specs/001-zkore-desktop-wallet/data-model.md`
- **Research**: `specs/001-zkore-desktop-wallet/research.md`
- **Quickstart**: `specs/001-zkore-desktop-wallet/quickstart.md`
- **Tasks**: `specs/001-zkore-desktop-wallet/tasks.md`

## Security Reporting and Incident Response

- Provide a private channel for vulnerability reports.
- Acknowledge reports promptly and track remediation steps internally.
- Public disclosure timing must balance user safety with transparency.

**Version**: 2.0.0 | **Ratified**: 2025-12-21 | **Last Amended**: 2026-01-09
