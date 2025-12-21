<!--
SYNC IMPACT REPORT
==================
Version change: 0.0.0 -> 1.0.0
Bump rationale: Initial adoption - MAJOR version for first ratification

Modified principles: N/A (initial creation)

Added sections:
  - Preamble
  - Core Principles (7 principles consolidated from 15 articles)
  - Non-Negotiable Checklist
  - Governance

Templates requiring updates:
  - .specify/templates/plan-template.md: Constitution Check section exists
  - .specify/templates/spec-template.md: No constitution references found
  - .specify/templates/tasks-template.md: No constitution references found

Source document: docs/constitution.md (15-article detailed version)

Follow-up TODOs: None
==================
-->

# Zkore Desktop Constitution

## Preamble

This document defines the non-negotiable principles governing Zkore Desktop development. These principles are consolidated from the detailed 15-article constitution in `docs/constitution.md`. If a feature, refactor, or integration conflicts with these principles, it does not ship until the conflict is resolved or this constitution is amended.

For detailed implementation rules, enforcement specifics, and article-by-article guidance, refer to the full constitution at `docs/constitution.md`.

## Core Principles

### I. Secrets Stay in Rust

The Rust backend is the single trust boundary for all secret material. The UI MUST NOT store, log, or compute with spending keys or raw seeds.

**Permitted mnemonic flows (with strict constraints):**
- Mnemonic MAY be sent to UI ONLY for: initial creation display, backup verification, and restore entry
- UI MUST NOT persist mnemonic to durable storage, MUST NOT log it, MUST clear from memory after flow completes
- Backend MUST NOT re-send mnemonic after initial creation response

**Permitted payload flows:**
- Raw unsigned/signed payloads MAY cross IPC ONLY for external signing flows (Keystone PCZT)
- Software wallet flows MUST use proposal-based pattern (tx bytes stay in backend)

**Prohibited flows:**
- Backend MUST NEVER send: raw seeds (entropy bytes), spending keys, tx bytes for software wallets
- UI MUST NEVER persist secrets, log mnemonic/seeds/payloads, or retain payloads beyond active session
- Memory containing secrets MUST use zeroization where feasible
- Logs MUST redact seeds, keys, full payloads, and raw memos by default

### II. Orchard-Only Privacy

All spending operations MUST use the Orchard shielded pool. Transparent funds MAY be received and displayed but MUST NOT be spent directly.

**Enforceable rules:**
- Default receive address MUST NOT include a transparent receiver
- Transparent funds require explicit shielding before becoming spendable
- Any privacy downgrade MUST be explicit, scoped, and user-acknowledged
- No Sapling spending, no transparent spending (receive-only for compatibility)

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
- CI MUST cover at least two server implementations (Zaino + lightwalletd)

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

## Non-Negotiable Checklist

Before merging work that touches wallet, signing, networking, or persistence, confirm:

- [ ] Secrets cannot reach the UI
- [ ] Logs remain redacted
- [ ] Transparent spending is still impossible
- [ ] Tor mode cannot silently downgrade
- [ ] IPC types are versioned and validated
- [ ] Migrations are tested
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

- **Detailed constitution**: `docs/constitution.md` (full 15-article version with implementation specifics)
- **Feature specifications**: `docs/spec.md`
- **Implementation plan**: `docs/plan.md`

**Version**: 1.0.0 | **Ratified**: 2025-12-21 | **Last Amended**: 2025-12-21
