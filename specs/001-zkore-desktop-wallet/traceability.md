# Traceability: Zkore Desktop Wallet

This document provides an explicit, review-friendly mapping from requirements in `spec.md` to implementation work in `tasks.md`.

## User Stories → Functional Requirements (FR)

| User Story | Related FR IDs |
|---|---|
| **US1** Create New Wallet and Receive Funds | FR-001, FR-002, FR-003, FR-004, FR-008a, FR-008b, FR-015 |
| **US2** Send Shielded Transaction with Memo | FR-009, FR-009a, FR-012, FR-013, FR-014 |
| **US3** Shield Transparent Funds | FR-010, FR-011 |
| **US4** Restore Wallet from Seed Phrase | FR-005, FR-006, FR-007, FR-008 |
| **US5** Receive to Fresh Shielded Address | FR-015, FR-016, FR-017, FR-018, FR-019 |
| **US6** Keystone Hardware Wallet Watch-Only | FR-020, FR-021 |
| **US7** Keystone Air-Gapped Signing | FR-022, FR-023, FR-024, FR-025, FR-026, FR-027, FR-028 |
| **US8** Swap To ZEC via NEAR Intents | FR-029, FR-033, FR-034, FR-035, FR-036 |
| **US9** Swap From ZEC via NEAR Intents | FR-030, FR-031, FR-032, FR-033, FR-034, FR-035, FR-036 |
| **US10** Enable Tor Anonymization | FR-037, FR-038, FR-039, FR-040, FR-041 |
| **US11** Wallet Status Widget | FR-042, FR-043, FR-044, FR-045, FR-046, FR-047 |
| **US12** Network Selection | FR-048, FR-049, FR-050, FR-051 |
| **Cross-story** Custom Servers | FR-052, FR-053, FR-054, FR-055 |

## Non-Functional Requirements (NFR) → Tasks

| NFR ID | Primary task coverage in `tasks.md` |
|---|---|
| **NFR-001** Local logs only | T060, T207, T208, T209 |
| **NFR-002** No telemetry/crash reporting | T209a, T209b |
| **NFR-003** User-accessible log location | T061, T208, T209 |
| **NFR-004** Log rotation | T060 |
| **NFR-005** Keyboard navigation | T058, T202, T204 |
| **NFR-006** ARIA labels | T201 |
| **NFR-007** Visible focus indicators | T203 |
| **NFR-008** Standard keyboard shortcuts | T059, T204 |
| **NFR-009** Encrypt spend-capable secrets at rest | T044, T044a, T066 |
| **NFR-010** Optional OS keychain “remember unlock” | T043a, T044d, T044e, T069 |
| **NFR-011** Locked on restart / unlock prompt | T043a, T056a |
| **NFR-012** Manual per-spend re-auth | T044c, T092, T097a, T103, T106b, T153, T163a |
| **NFR-013** Manual re-auth to view seed phrase | T044c, T068d, T084a |
| **NFR-014** Memo protection at rest / no memo leakage | T044b, T207, T212 |
| **NFR-015** Encrypt entire wallet DB at rest | T044b, T044b2 |
| **NFR-016** Migrations: forward + rollback + tests | T038a, T038b, T044b1, T044b2, T216c |

## Notes

- The task list is the canonical implementation plan; this file is intentionally minimal to keep review overhead low.
- Some FRs are shared across stories (e.g., swap Activity/state machine requirements); they are listed under each relevant story above.
