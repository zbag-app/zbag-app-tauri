# `CheckpointConflict` During Sync: Cause and Required Fixes

This note captures the practical invariants around commitment-tree checkpoints during block scanning and
the changes required in `zkore-engine` to prevent `CheckpointConflict` (and “100% synced but unusable” wallets).

## Symptoms

Typical logs look like:

```text
failed to scan blocks ... error=Wallet(CommitmentTree(Storage(CheckpointConflict { ... })))
```

If the sync loop logs this and continues, the wallet can incorrectly report “sync complete” while:

- transactions in the failed scan region are silently missed
- note witnesses are incomplete (spends may fail later)

## Why `CheckpointConflict` happens

The wallet database stores commitment-tree checkpoints keyed by height.
When calling `zcash_client_backend::data_api::chain::scan_cached_blocks(...)`, librustzcash will:

- use the provided `ChainState` to write/validate a checkpoint at `from_height - 1`
- expect that checkpoint state to be consistent with existing DB state

### Invariant: per-call `from_state` must be correct

`scan_cached_blocks(from_height, from_state, limit)` has a strict contract:

- `from_state` must describe the chain state at height `from_height - 1`

Passing an “empty” tree frontier for a height where the wallet DB already has a non-empty frontier will
cause a checkpoint write/validation conflict and produce `CheckpointConflict`.

## Common root causes in a sync loop

### 1) Batched scanning with incorrect `from_state`

If you scan a range in N-block batches but fetch a real `ChainState` only for the first batch (or worse,
use `ChainState::empty(...)` for later batches), subsequent calls will attempt to create checkpoints that
do not match the DB and can trigger `CheckpointConflict` (especially in incremental syncs).

### 2) `GetBlockRange` end-bound off-by-one (overlapping batches)

lightwalletd `GetBlockRange(start, end)` uses an **inclusive** end height, while Rust `Range` uses an
**exclusive** end.

If the downloader is passed an end-exclusive bound but forwards it directly to gRPC as an inclusive end,
adjacent batches will overlap by one block (`...200` appears in both `100..200` and `200..300`), which can
cause duplicate caching and downstream scan/continuity failures.

## What to change (required)

### A) Always pass the correct `ChainState` for each scan call

For every `scan_cached_blocks(...)` invocation:

1. Compute `prior_height = from_height.saturating_sub(1)`.
2. Obtain the `ChainState` for `prior_height`:
   - Height 0 can use an empty state.
   - Otherwise, fetch `TreeState` from lightwalletd and convert to `ChainState`.
3. If the tree state fetch/parsing fails (non-genesis), abort the range/sync.
4. If scanning fails, abort the range/sync (do not log-and-continue).

This matches the librustzcash contract and prevents “checkpoint drift” vs the wallet DB.

### B) Fix downloader semantics to avoid overlapping block ranges

Pick one convention and enforce it:

- Internally: treat downloaded block ranges as `start..end_exclusive`.
- When calling lightwalletd: translate to inclusive:
  - `end_inclusive = end_exclusive - 1` (guard `end_exclusive > start`)

This ensures no overlap or duplication at batch boundaries.

## Recommended implementation shape

Prefer a batched scan loop (good cancellation/progress characteristics), but keep the invariants above:

- download batch `start..end_exclusive`
- write to block cache
- fetch `ChainState` for `start - 1`
- call `scan_cached_blocks(..., start, &chain_state, limit = batch_len)`
- on success, truncate cache up to `start - 1`
- on failure, abort (and let the next sync attempt retry cleanly)

## Verification

After applying the changes:

- incremental sync does not emit `CheckpointConflict`
- scan failures do not produce a “100% complete” state
- a spend-building flow (e.g., harness “send to self”) succeeds after sync completes

