Overall, the repo is very close to internally consistent: the constitution, spec, plan, IPC contract, and most of the task breakdown reinforce the same security and product decisions.

That said, there are a few concrete misalignments that will cause either (a) an implementation dead end for a P1 story, or (b) a requirement not being fully met as written. I would not treat this as fully aligned until the items below are corrected.

## Corrections needed

### 1) P1 shielding story cannot be completed with the current address tasks ordering

**What is misaligned**

* In `specs/001-zkore-desktop-wallet/spec.md`, User Story 3 (P1) requires that users can receive funds to a transparent address (compatibility) and then shield them.
* In `specs/001-zkore-desktop-wallet/tasks.md`, transparent address support is deferred to User Story 5 (P2):

  * `T071` explicitly says to reject `AddressType::Transparent` for US1.
  * `T117` and `T119` (transparent derivation + UI toggle) are placed in US5.
* This makes US3’s “independent test” and acceptance scenarios practically impossible during the MVP sequence (US1–US3).

**What to change**

File: `specs/001-zkore-desktop-wallet/tasks.md`

1. Update `T071` (backend address service) so US1-US3 can access a transparent compatibility address:

* Replace the “reject AddressType::Transparent” behavior with:

  * `AddressType::ShieldedOnly`: UA with Orchard+Sapling only (no transparent receiver).
  * `AddressType::Transparent`: return a single stable transparent address per account (no rotation in v1).
* Keep shielded address rotation as a later enhancement (US5).

2. Move or duplicate the UI work needed to expose the transparent compatibility address before US3 is “done”:

* Move `T119` (transparent compatibility toggle in `apps/zkore-app-tauri/src/pages/Receive.tsx`) from US5 into US1 or US3.
* Ensure the Receive screen includes the labeling/explanation required by FR-018, and that the transparent address is clearly marked as “compatibility”.

3. Adjust US5 to focus on rotation instead of introducing transparent support:

* In the US5 section, remove (or mark as already completed earlier) the tasks that only exist to introduce transparent support:

  * `T117` (transparent address derivation)
  * `T119` (toggle)
* Leave US5 to implement:

  * diversifier rotation tracking (`T072` / `receive_rotation`)
  * “fresh shielded address on each Receive open” behavior
  * any refactor components if still useful (`AddressDisplay.tsx` etc)

4. Update US3 milestone test scope to include the actual way a user gets transparent funds:

* In `T106a` (US3 tests), add coverage that:

  * the transparent compatibility address can be retrieved from the Receive flow
  * sending to that address results in `transparent_total > 0`
  * those funds remain blocked from direct spending and require shielding

This is the single biggest alignment issue because it blocks a P1 story from being independently deliverable as written.

---

### 2) Tor status “at all times” is not fully accounted for in the UI tasks

**What is misaligned**

* `specs/001-zkore-desktop-wallet/spec.md` includes **FR-038**: Tor status must be displayed “at all times” (Off/Connecting/On/Error).
* `specs/001-zkore-desktop-wallet/tasks.md` under US10 includes:

  * settings toggle (`T179`)
  * badge component (`T180`)
  * error dialog (`T181`)
* But there is no task that places the Tor status indicator in global UI chrome across screens. Putting it only on Settings does not satisfy “at all times” in the usual interpretation.

**What to change**

File: `specs/001-zkore-desktop-wallet/tasks.md`

Add a task in US10 (or frontend foundation) that explicitly renders Tor status globally:

* Example change: add a new task (near `T179`/`T180`) such as:

  * “Render `TorStatusBadge` in the persistent app header/layout (for example in `apps/zkore-app-tauri/src/App.tsx` or a shared layout component) so it is visible on all pages.”
* Ensure the app:

  * calls `GetTorState` on startup to initialize the badge
  * subscribes to `TorStatusEvent` for real-time updates

Also add an e2e assertion for this (US10 e2e file) so the requirement stays enforced.

---

### 3) Ambiguity and inconsistency around `ZKORE_NETWORK` in `.env.development`

**What is misaligned**

* `specs/001-zkore-desktop-wallet/quickstart.md` and task `T010` define `.env.development` with `ZKORE_GRPC_URL`, `ZKORE_NETWORK`, and `RUST_LOG`.
* `specs/001-zkore-desktop-wallet/plan.md` and `research.md` only call out `ZKORE_GRPC_URL` as the override mechanism.
* The presence of a global `ZKORE_NETWORK` variable is easy to misimplement in a way that conflicts with the repo’s core rule: network is chosen at wallet creation and is immutable, and multiple wallets can exist across networks.

**What to change**

Pick one and make the docs consistent:

Option A (simplest):

* Remove `ZKORE_NETWORK` from:

  * `specs/001-zkore-desktop-wallet/quickstart.md` (the `.env.development` example)
  * `specs/001-zkore-desktop-wallet/tasks.md` task `T010`
* Keep only:

  * `ZKORE_GRPC_URL` as a dev override
  * `RUST_LOG`

Option B (if you want to keep an env-scoped network for the override):

* Rename `ZKORE_NETWORK` to something that clearly scopes only the override server, not wallet behavior, for example `ZKORE_GRPC_NETWORK`.
* Clarify allowed values and casing, and explicitly state:

  * it does not override `wallet.network`
  * it only scopes which network the override endpoint corresponds to during development

If Option B is chosen, update `plan.md` (Server Configuration section) and/or `research.md` to mention this env var so the design docs match the quickstart.

---

### 4) Persisted broadcast retry queue is required by plan/tasks but not represented in the data model

**What is misaligned**

* `specs/001-zkore-desktop-wallet/tasks.md` includes `T094a` (persisted broadcast queue with signed tx bytes stored encrypted, retained up to 7 days, user-initiated retry only).
* `specs/001-zkore-desktop-wallet/plan.md` also describes this behavior.
* `specs/001-zkore-desktop-wallet/data-model.md` does not define any entity/table/file-based storage model for this persisted queue and currently frames “avoid storing raw payloads” without documenting this required exception.

**What to change**

File: `specs/001-zkore-desktop-wallet/data-model.md`

* Add a small entity section describing the persisted broadcast queue (name it explicitly, for example `QueuedBroadcast`).

  * Include: txid, created_at, last_error, encrypted tx bytes location, expires/retention policy (7 days), and deletion on success.
  * Explicitly state it is encrypted-at-rest (either by wallet DB encryption or by explicit DEK-based encryption if stored as files).

Then ensure the “Database Schema Overview” reflects where this lives:

* If it’s a new table in app metadata DB or wallet DB, add it to the schema overview and mention migration/testing implications.
* If it’s encrypted file blobs in the wallet directory, document that location and what metadata is persisted and where.

File: `specs/001-zkore-desktop-wallet/tasks.md`

* If you decide it needs a new DB table, add migration + rollback + tests tasks for it (so it stays compliant with NFR-016).
* If it’s file-based, add a short note in the task describing where the encrypted queue files live and what tests validate retention cleanup.

---

### 5) Keep the constitution self-contained

**What to change**

File: `.specify/memory/constitution.md`

* Remove any references to an external “supplement” file so the constitution remains self-contained and canonical.

---

### 6) Clarify “active wallet” semantics across CreateWallet/RestoreWallet vs LoadWallet

**What is misaligned**

* `specs/001-zkore-desktop-wallet/contracts/ipc-v1.ts` states that account-scoped commands operate on the currently loaded wallet set by `LoadWallet`.
* `specs/001-zkore-desktop-wallet/quickstart.md` repeats: call `LoadWallet` before account-scoped calls.
* But tasks and flows do not clearly specify what happens immediately after `CreateWallet` or `RestoreWallet`:

  * Does `CreateWallet` also set the created wallet as the active wallet?
  * Or must the UI call `LoadWallet` right after create/restore?

This is not just stylistic, it affects whether the UI can call `GetReceiveAddress`, `GetBalance`, `ListTransactions`, and subscribe to events immediately after create/restore.

**What to change**

Make it explicit in one place, then align the others.

Recommended approach (simple UX):

* Define that `CreateWallet` and `RestoreWallet` set the new wallet as the active wallet, equivalent to a load.

Concrete edits:

* File: `specs/001-zkore-desktop-wallet/contracts/ipc-v1.ts`

  * Add a short comment under `CreateWallet` and `RestoreWallet` describing whether they set the active wallet.
* File: `specs/001-zkore-desktop-wallet/quickstart.md`

  * Update the note about account-scoped requests to mention the exception/behavior after create/restore.
* File: `specs/001-zkore-desktop-wallet/tasks.md`

  * Add a line in the CreateWallet/RestoreWallet command tasks stating that the backend must set the active wallet (or alternatively add a frontend task to call `LoadWallet` immediately after create/restore if that’s the chosen design).

---

## Recommendations

These are not strict blockers, but they improve traceability and reduce implementation drift.

1. **Update quickstart CI guidance to include the constitution’s “two independent lightwalletd deployments” requirement**

* File: `specs/001-zkore-desktop-wallet/quickstart.md`

  * In the “CI Pipeline Requirements” snippet, add a note that integration tests must run against at least two independent lightwalletd deployments, matching constitution Principle V and tasks `T216b`.

2. **Swap asset list source**

* Research notes `GET /v0/tokens` exists for 1Click.
* Tasks do not define how Swap asset selection is populated.
* Either:

  * Add a task to implement a “supported tokens list” fetch and cache in backend, or
  * Explicitly document a static list approach for v1 (and where it lives) to prevent ambiguity.

3. **Front-end logging hygiene**

* You already have strong backend redaction tasks and tests.
* Consider adding a small note or lint rule guidance (even in `AGENTS.md`) discouraging `console.log` of IPC payloads containing memos, seed words, signing frames, etc. This is consistent with the constitution’s general “logs must redact sensitive content”.

---

## Bottom line

Most of the documents line up well, but there are several concrete alignment fixes needed before you can treat the set as fully consistent, especially the US3 transparent address dependency and the Tor status visibility requirement.
