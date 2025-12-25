## Verdict

Not a green light yet.

Most of the spec, plan, tasks, data model, and IPC contract are internally consistent and constitution-aligned. The main blocker is that multiple documents still treat Zaino as a supported/target server (and as a CI compatibility axis), which conflicts with your stated direction: Zkore should talk to lightwalletd only (lightwalletd + Zebra behind it) and ignore Zaino. There are also a couple smaller internal consistency issues in the tasks/doc flow that should be fixed to avoid implementation ambiguity.

Below are the specific corrections to make.

---

## Required corrections

### 1) Remove Zaino everywhere and replace with “lightwalletd only”

You should do a repo-wide search for `Zaino` and remove or rewrite every occurrence. The edits below are the ones I can confirm from the provided files.

#### A) `.specify/memory/constitution.md`

**Problem:** Principle V and the Non-Negotiable Checklist hard-require CI coverage of “Zaino + lightwalletd”. That is incompatible with “ignore Zaino”.

**Change:**

1. In **Core Principles -> V. Test-Driven Quality**:

* Replace the bullet:

  * `CI MUST cover at least two server implementations (Zaino + lightwalletd)`
* With something like:

  * `CI MUST cover at least two independent lightwalletd deployments (primary + secondary) to catch server-side behavior differences and regressions.`

2. In **Non-Negotiable Checklist**:

* Replace:

  * `CI MUST cover at least two server implementations (Zaino + lightwalletd)`
* With:

  * `CI MUST cover at least two independent lightwalletd deployments (primary + secondary).`

3. Because this is a constitution amendment (principle-level change), also update the footer:

* Bump **Version** (this is a material change to Principle V, so a MINOR bump makes sense, for example `1.3.0`).
* Update **Last Amended** date.

Optional but recommended: update the “SYNC IMPACT REPORT” header comment to reflect this amendment (currently it documents a different change).

#### B) `specs/001-zkore-desktop-wallet/spec.md`

**Problem:** The spec explicitly names Zaino as a dependency and mentions lightwalletd/Zaino in custom server requirements.

**Change:**

1. In **Requirements -> Network Selection and Custom Servers**:

* Update **FR-052** from:

  * `System MUST allow users to configure custom lightwalletd/Zaino server URLs`
* To:

  * `System MUST allow users to configure custom lightwalletd server URLs`

2. In **Dependencies**:

* Remove the bullet:

  * `Zaino (Rust indexer, lightwalletd-compatible) as alternative to lightwalletd`
* Keep “CompactTxStreamer-compatible light client server” language, but do not name Zaino.

#### C) `specs/001-zkore-desktop-wallet/plan.md`

**Problem:** The plan repeatedly treats Zaino as a supported compatibility target and repeats the CI requirement.

**Change:**

1. In **Technical Context** (Testing line):

* Replace:

  * `integration tests against Zaino/lightwalletd endpoints`
* With:

  * `integration tests against lightwalletd endpoints (at least two independent deployments in CI).`

2. In **Server Configuration** section:

* Remove all “Zaino migration is in progress” and “Zaino endpoints” language.
* Replace any “lightwalletd/Zaino endpoint” references with “lightwalletd endpoint”.

3. Anywhere the plan says CI must test “both lightwalletd and Zaino”:

* Replace with “two independent lightwalletd deployments”.

#### D) `specs/001-zkore-desktop-wallet/research.md`

**Problem:** Research section 12 is written as if Zaino is a supported alternative and as if CI must validate Zaino compatibility.

**Change:**

1. In **Topic 12. Server Configuration**:

* Remove the entire “Zaino endpoints (experimental)” block and any “Zaino compatibility” text.
* Rewrite the server story as:

  * Zkore talks to lightwalletd (CompactTxStreamer gRPC).
  * lightwalletd talks to Zebra (not directly relevant to the app).
  * Custom servers mean custom lightwalletd endpoints only.

2. In **Topic 12 references**:

* Remove:

  * `Zaino GitHub`
* Remove any “check zec.rocks announcements for Zaino” language.

3. Anywhere research says:

* “Constitution requires testing against multiple server implementations (Zaino + lightwalletd)”
* Rewrite to:

  * “Constitution requires testing against multiple independent lightwalletd deployments”.

#### E) `specs/001-zkore-desktop-wallet/tasks.md`

**Problem:** Tasks explicitly call out Zaino in networking behavior and CI/test requirements.

**Change the following tasks:**

1. **T048a** (mempool support):

* Current text mentions enabling pending detection “on both lightwalletd and Zaino”.
* Update to something like:

  * “Add CompactTxStreamer mempool support (stream or polling, depending on lightwalletd server support) to enable pending-transaction detection (FR-013).”

2. **T100a** (US2 milestone tests):

* Replace:

  * “run against both lightwalletd and Zaino in CI”
* With:

  * “run against at least two independent lightwalletd deployments in CI (primary + secondary).”

3. **T216b** (CI matrix):

* Replace the whole Zaino/lightwalletd matrix requirement with:

  * “Add CI matrix coverage across at least two independent lightwalletd deployments (primary + secondary) and fail if either backend fails.”

Also check the rest of tasks.md for any other “Zaino” occurrences and remove them.

#### F) Quick sweep for other files

Even if not shown as containing Zaino in the excerpts, the coding agent should still run:

* A repo-wide search for `Zaino`
* And remove/replace every occurrence in:

  * `README.md`
  * `CLAUDE.md`
  * `AGENTS.md`
  * `quickstart.md`
  * any CI templates or scripts

Your stated target should be consistent everywhere: “lightwalletd only”.

---

### 2) Fix a real tasks-to-spec alignment gap: US1 needs wallet backup state on Home

**Problem:** US1 requires a persistent backup reminder. The IPC contract already includes `GetWalletStatus` returning `backup_status`. But tasks schedule the actual `GetWalletStatus` implementation under US11 (Phase 13). That leaves US1 without a clean way to know “backup required” on startup/reopen, which is needed for the persistent reminder.

**Correction options (pick one, but do not leave it ambiguous):**

#### Option A (recommended): implement `GetWalletStatus` in US1

In `specs/001-zkore-desktop-wallet/tasks.md`:

1. Add a US1 task (or move/retag the existing one) so that by end of US1 you have:

* Backend: `zkore_get_wallet_status` implemented
* It returns at minimum:

  * `lock_status`
  * `backup_status` (Required/Complete)
  * `sync_status` can be basic (Synced or Syncing) based on what US1 already implements
  * `shield_status` can be `None` in US1 (since shielding is US3)
  * `privacy_posture` derived from backup_status (NeedsAction if backup required, otherwise Optimal)

2. Update US1 UI tasks (`Home.tsx`, `BackupReminder.tsx`) to call `zkore_get_wallet_status` and drive the reminder from `status.backup_status === 'Required'`.

Then in US11, you focus on the widget UI and eventing, and you can enhance the computation (transparent funds, etc) after US3 exists.

#### Option B: extend `LoadWalletResponse` to include backup status (not recommended)

This requires changing `contracts/ipc-v1.ts` and all Rust IPC types. It is more invasive and breaks the clean separation already designed around `GetWalletStatus`.

Given you already have the command in the contract, Option A is the cleaner fix.

---

### 3) Remove or resolve the duplicate “BackupAction” domain type plan in tasks

**Problem:** In tasks.md:

* **T022** creates `domain/backup.rs` with `BackupStatus` and `BackupAction`
* **T182** later creates `domain/wallet_status.rs` with `WalletStatus` and `BackupAction`

That sets you up for naming collisions or duplicate types.

**Correction:**

Pick one canonical home for `BackupAction` and update tasks.md accordingly.

Recommended approach:

* Keep `BackupStatus` (the entity-like state) in `crates/zkore-core/src/domain/backup.rs`
* Define `BackupAction` only in `crates/zkore-core/src/domain/wallet_status.rs` (because it is explicitly part of the WalletStatus widget model and matches IPC)

So update **T022** to:

* “Create crates/zkore-core/src/domain/backup.rs with BackupStatus type (and any backup verification metadata types), but do not define BackupAction here.”

---

## Recommended improvements (not strictly blockers, but they prevent future drift)

### 1) Add a simple traceability matrix document

You asked for checklist items to trace to requirements. Right now, that trace is implicit (tasks grouped by story, spec has FR/NFR lists), but not explicit.

Recommendation:

* Add `specs/001-zkore-desktop-wallet/traceability.md` with:

  * Each FR/NFR
  * The tasks (T-ids) that implement it
  * The test files that cover it (unit/integration/e2e)

This will make review and implementation gating much easier, especially for security items.

### 2) Standardize the “network directory name” convention across docs

Some places use `~/.zkore/wallets/{network}/...` while others show `mainnet`/`testnet`.

Recommendation:

* In plan.md + data-model.md + tasks.md, explicitly define:

  * `Network::Mainnet` maps to directory slug `mainnet`
  * `Network::Testnet` maps to directory slug `testnet`

Then update any ambiguous instances like `~/.zkore/wallets/{network}/...` to `~/.zkore/wallets/{network_slug}/...`.

### 3) (Optional) Fill the “BalanceChangedEvent” gap if the architecture expects it

The IPC contract and quickstart examples include a `BalanceChangedEvent`, but tasks do not explicitly include emitting it anywhere.

Recommendation:

* Add a task in US1 or US4 to emit `balance.changed` on balance updates, with a small integration test to confirm the channel payload shape.

---

## After these changes

Once:

* All Zaino references are removed and replaced with lightwalletd-only language (including constitution and CI tasks),
* US1 has a concrete backend source of truth for backup status on Home (preferably `GetWalletStatus` implemented in US1),
* The BackupAction type duplication in tasks is resolved,

Then I would consider the docs aligned enough to proceed with implementation.

