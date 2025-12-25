## Status

There are a handful of real alignment gaps that should be corrected before implementation starts. Most of the design is internally consistent, but the items below will otherwise create confusion or force avoidable refactors.

## Corrections needed

### 1. IPC command return shape is inconsistent across contract vs quickstart vs tasks

**Problem**

* `specs/001-zkore-desktop-wallet/contracts/ipc-v1.ts` defines a first-class `IpcResult<T> = { ok: T } | { err: IpcError }`.
* `specs/001-zkore-desktop-wallet/quickstart.md` shows Tauri commands returning `Result<Response, IpcError>` and TS wrappers returning `Promise<Response>`.
* `specs/001-zkore-desktop-wallet/tasks.md` includes “IpcResult” as a foundational type (T028) but later tasks and quickstart examples don’t actually use it as the command boundary.

This is the biggest mismatch because it affects every command signature and every frontend invoke wrapper.

**Recommended resolution (pick one and make all docs match)**
I recommend standardizing on the contract as written: every command returns `IpcResult<Response>` over IPC (never throws across the boundary), and the UI handles `{ ok } | { err }`.

**Precise changes**

1. Update `specs/001-zkore-desktop-wallet/quickstart.md`:

* In “Step 2: Tauri Commands Skeleton”, change command return types from:

  * `Result<CreateWalletResponse, IpcError>`
  * `Result<GetBalanceResponse, IpcError>`
    to:
  * `IpcResult<CreateWalletResponse>`
  * `IpcResult<GetBalanceResponse>`

* In “Step 3: Frontend IPC Client”, change wrapper return types from `Promise<...Response>` to `Promise<IPC.IpcResult<...Response>>`.

Example edits (illustrative, not implementation):

```rust
// quickstart.md
#[tauri::command]
pub async fn zkore_create_wallet(
    state: State<'_, AppState>,
    request: CreateWalletRequest,
) -> IpcResult<CreateWalletResponse> {
    todo!()
}
```

```ts
// quickstart.md
export async function createWallet(
  request: IPC.CreateWalletRequest
): Promise<IPC.IpcResult<IPC.CreateWalletResponse>> {
  return invoke(IPC.Commands.CREATE_WALLET, { request });
}
```

2. Update `specs/001-zkore-desktop-wallet/tasks.md`:

* In Phase 2.2 (IPC Contracts), add a single explicit line to T028 (or T028a) stating the convention: “All Tauri commands return `IpcResult<Response>` (no thrown errors across IPC); frontend wrappers must return `IpcResult<T>`.”

3. Update `specs/001-zkore-desktop-wallet/plan.md` and/or `specs/001-zkore-desktop-wallet/research.md`:

* Add one short “IPC error handling convention” note that matches the above so future readers don’t reintroduce `Result<T, E>` at the boundary.

If you prefer the opposite convention (Tauri `Result<T, E>` with thrown errors), then remove `IpcResult<T>` from `ipc-v1.ts`, remove it from the Rust IPC common types, and update every reference in tasks/quickstart accordingly. But do not keep both patterns documented at the same time.

---

### 2. Event channel list is missing `wallet-status` in multiple docs

**Problem**

* The contract defines `EventChannels.WALLET_STATUS = 'zkore://wallet-status'` and an event type `wallet.status`.
* `specs/001-zkore-desktop-wallet/research.md` lists event channels but omits `wallet-status`.
* `specs/001-zkore-desktop-wallet/quickstart.md` notes event channels but omits `wallet-status`.
* `specs/001-zkore-desktop-wallet/plan.md` similarly omits it in the IPC architecture notes.

**Precise changes**

* `specs/001-zkore-desktop-wallet/research.md`

  * In “5. Tauri v2 IPC Architecture” Implementation Notes, update the event channel list to include `wallet-status`.

* `specs/001-zkore-desktop-wallet/quickstart.md`

  * In the IPC/event notes section, include `wallet-status` alongside the other channels.

* `specs/001-zkore-desktop-wallet/plan.md`

  * Anywhere it enumerates event channels, add `wallet-status`.

---

### 3. `.env.development` variables differ across docs and tasks

**Problem**

* `specs/001-zkore-desktop-wallet/quickstart.md` includes `ZKORE_GRPC_URL`, `ZKORE_NETWORK`, and `RUST_LOG`.
* `AGENTS.md` also expects `ZKORE_NETWORK`.
* `specs/001-zkore-desktop-wallet/tasks.md` T010 only mentions `ZKORE_GRPC_URL` and `RUST_LOG`.

**Precise change**

* Update `specs/001-zkore-desktop-wallet/tasks.md` T010 to include `ZKORE_NETWORK` as part of the required `.env.development` content.

---

### 4. US1 receive-address task is underspecified relative to the spec and data model

**Problem**

* The spec and data model define the default receive address as a shielded-only Unified Address (Orchard + Sapling, no transparent receiver).
* `specs/001-zkore-desktop-wallet/tasks.md` T071 currently says “minimal shielded receive address support” and implies AddressType handling comes later in US5, even though `ipc-v1.ts` already includes `address_type` in the request and the domain model defines `AddressType = ShieldedOnly | Transparent`.

This risks someone implementing “shielded” as a Sapling-only address in US1 and deferring UA generation to US5, which would violate FR-015 and the data model.

**Precise changes**

* Update `specs/001-zkore-desktop-wallet/tasks.md` T071 to explicitly require:

  * Shielded-only Unified Address generation (Orchard + Sapling receivers, no transparent receiver) for `AddressType::ShieldedOnly`.
  * For US1, either reject `AddressType::Transparent` with a stable error (likely `INVALID_REQUEST`), or document that Transparent support is implemented in US5.

* Update `specs/001-zkore-desktop-wallet/tasks.md` T118 wording:

  * Replace “Update GetReceiveAddress to support AddressType parameter” with something like:

    * “Implement Transparent address support + diversifier rotation; previously only ShieldedOnly was supported.”

This keeps the request shape consistent from day one while allowing feature-gated behavior.

---

### 5. ShieldFunds command task does not reflect the IPC request shape

**Problem**

* `ipc-v1.ts` defines `ShieldFundsRequest` with:

  * `account_id`
  * `consolidate`
  * `reauth_token`
* `specs/001-zkore-desktop-wallet/tasks.md` T103 currently says it accepts reauth_token, but does not mention `account_id` or `consolidate`.

**Precise changes**

* Update `specs/001-zkore-desktop-wallet/tasks.md` T103 to state it accepts the full request fields per `ipc-v1.ts`.
* Optional but recommended: add a note in US3 tasks that in v1 the UI always sets `consolidate = true` (since the product requirement is “Shield and Consolidate”).

If you do not want a consolidate toggle in v1 at all, remove `consolidate` from `ipc-v1.ts`, from the Rust IPC types, and from the tasks. Right now the contract includes it, so the tasks should acknowledge it.

---

### 6. IPC “no secrets in payloads” regression-test description omits the backup-verification flow

**Problem**

* `specs/001-zkore-desktop-wallet/tasks.md` T028b says to add a regression check that IPC payloads never include mnemonic/seed words except in CreateWallet, RestoreWallet, ViewSeedPhrase.
* But the contract includes `VerifyBackupRequest.word_challenges`, which necessarily carries seed words (4 words) from UI to backend, and the constitution explicitly allows backup verification flows.

**Precise change**

* Update the T028b description to explicitly allow `VerifyBackupRequest.word_challenges` as an additional permitted seed-word flow, and tighten the rule to “no seed words appear in any backend-to-UI payloads except CreateWalletResponse and ViewSeedPhraseResponse.”

This keeps the intent (no accidental leakage) without contradicting backup verification.

---

### 7. Spec status metadata conflicts with the “ready” checklist

**Problem**

* `specs/001-zkore-desktop-wallet/spec.md` header says “Status: Draft”.
* `specs/001-zkore-desktop-wallet/checklists/requirements.md` claims everything passes and the spec is ready.

**Precise change**

* Update `specs/001-zkore-desktop-wallet/spec.md` to “Status: Complete” (or “Ready”) to match the checklist, or update the checklist to explicitly say “Draft but passes checklist” (less ideal).

---

### 8. Rust toolchain/MSRV wording is internally confusing

**Problem**

* Several places say dev toolchain is 1.92.0 but also mention MSRV 1.85.1 compatibility.
* The quickstart’s example workspace manifest sets `rust-version = "1.92.0"`, which implies MSRV 1.92.0, not 1.85.1.

**Recommended resolution**
Unless you are explicitly going to enforce MSRV in CI, treat 1.85.1 as informational about librustzcash, not a project guarantee.

**Precise changes**

* `specs/001-zkore-desktop-wallet/plan.md`

  * In “Edition 2024 Rationale” (and any other place), change wording so it does not claim the project maintains MSRV 1.85.1 unless you actually intend to enforce it.

* `specs/001-zkore-desktop-wallet/research.md`

  * In the “Resolved Clarifications” table row for Rust version, adjust similarly.

If you do intend to enforce MSRV 1.85.1:

* Change quickstart’s example `rust-version` to `1.85.1`, keep `rust-toolchain.toml` pinned to 1.92.0, and add an explicit CI task to build with MSRV. Right now, the docs do not include that enforcement step.

---

## Recommendations

These are not strict misalignments, but they will reduce implementation risk.

1. Add a lightweight trace matrix
   Create a file like `specs/001-zkore-desktop-wallet/traceability.md` mapping:

* FR/NFR IDs -> task IDs -> test files (unit/integration/e2e)
  This will make “what covers what” obvious and prevent missing requirements.

2. Clarify memo-at-rest wording to avoid misinterpretation
   Across `spec.md`, `plan.md`, and `data-model.md`, the phrase “memo plaintext MUST NOT be written to disk” is easy to read as “never stored at rest”, even though the design clearly intends encrypted-at-rest storage. Consider tightening to “must not be written to disk unencrypted” for clarity.

3. In quickstart dependency list, explicitly enable XChaCha20 support if required
   If the planned implementation uses `XChaCha20-Poly1305`, ensure the quickstart’s dependency line includes the needed feature flags (if applicable) so the first build doesn’t fail for new developers.

4. Minor hygiene: task numbering
   `T072` appearing later than `T073` is not functionally wrong, but it is easy to misread. Consider renumbering or adding a brief note that numbering is unique but not strictly sequential.

---

## After the above changes

Everything else is broadly consistent: the constitution constraints are reflected in the spec, plan, data model, IPC contract, and task breakdown, and the testing posture in tasks matches the stated quality gates. The listed corrections are the ones most likely to trip up an implementation agent if left unresolved.

