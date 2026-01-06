# CheckpointConflict Bug: Analysis and Fix

## Executive Summary

Incremental wallet syncs were failing with `CheckpointConflict` errors, causing transactions to be silently missed. The root cause was an incorrect optimization that passed empty commitment tree state to librustzcash for non-first batches during block scanning. This document explains the bug mechanics, why the original approach was flawed, and how the fix resolves it.

---

## The Problem

### Symptoms

During incremental syncs (syncing a wallet that already has data), the logs showed:

```
WARN zkore_engine::sync_service: failed to scan blocks
  wallet_id=fb483d9f-62f3-4b74-85c1-d1ffd1a3acb3
  range_start=461004
  limit=101
  error=Wallet(CommitmentTree(Storage(CheckpointConflict {
    checkpoint_id: BlockHeight(461003),
    checkpoint: Checkpoint { tree_state: Empty, marks_removed: {} },
    extant_tree_state: AtPosition(Position(61450)),
    extant_marks_removed: None
  })))
```

The sync would continue past these errors, report "Sync complete!", but transactions in failed batches were never processed.

### Impact

| Consequence | Severity |
|-------------|----------|
| Missed transactions | High |
| Incomplete note witnesses | Medium |
| Unspendable funds (missing witnesses) | Medium |
| False "sync complete" status | High |

---

## Technical Background

### How Zcash Note Commitment Trees Work

Zcash shielded transactions use cryptographic commitment trees (Sapling and Orchard) to prove note ownership without revealing transaction details. When scanning blocks:

1. Each shielded output adds a commitment to the tree
2. The tree's "frontier" (rightmost path) is needed to compute witnesses
3. Witnesses prove a note exists in the tree at spending time
4. Without valid witnesses, notes cannot be spent

### The ChainState Parameter

When calling `scan_cached_blocks()`, librustzcash requires a `ChainState` parameter containing:

- Block height and hash
- Sapling commitment tree frontier
- Orchard commitment tree frontier

This state serves two purposes:

1. **Witness computation**: The scanner needs tree frontiers to compute witnesses for received notes
2. **Checkpoint validation**: librustzcash creates checkpoints in the wallet database to enable rollbacks; these checkpoints must be consistent with existing data

---

## Root Cause Analysis

### The Flawed Optimization

The sync service processes blocks in batches of 100. For efficiency, the original code fetched tree state only once per "range" (which could span thousands of blocks):

```rust
// Original code (sync_service.rs lines 544-550)
let prior_height = batch.range_start.saturating_sub(1);
let chain_state = if is_first_batch_in_range {
    is_first_batch_in_range = false;
    range_chain_state.clone()  // Fetched once at range start
} else {
    empty_chain_state(prior_height)  // BUG: Empty state for subsequent batches
};
```

The comment claimed: "the scanner maintains internal state between calls so it doesn't need the tree state again."

### Why This Assumption Was Wrong

The assumption conflates two different concepts:

| Concept | What It Is | Maintained Between Calls? |
|---------|------------|---------------------------|
| Scanner state | Internal tracking of scan progress | Yes |
| ChainState for checkpoints | Tree frontiers for wallet DB consistency | No - must match DB state |

While the scanner does track its progress internally, librustzcash uses the `ChainState` parameter independently for checkpoint management. When we pass `empty_chain_state`:

1. We declare "the commitment tree is empty at height N"
2. librustzcash attempts to create a checkpoint with `tree_state: Empty`
3. The wallet database already has tree data at position 61450 (from previous syncs)
4. Conflict: empty vs. non-empty tree state at the same checkpoint

### Fresh Sync vs. Incremental Sync

| Scenario | Has Existing Data? | Empty State Valid? |
|----------|-------------------|-------------------|
| Fresh sync (new wallet) | No | Yes - tree genuinely starts empty |
| Incremental sync | Yes | No - conflicts with stored checkpoints |

This explains why the bug only manifested during incremental syncs.

### The Silent Failure Problem

Compounding the issue, scan failures were logged but not acted upon:

```rust
// Original error handling (lines 581-590)
Err(err) => {
    tracing::error!(..., "failed to scan blocks - transactions in this range may be missed");
    // Continue - partial scan is ok, but error is logged  <-- WRONG
}
```

This meant:
1. Batch N fails with CheckpointConflict
2. Code continues to batch N+1
3. Batch N+1 also uses empty state, also fails
4. Pattern repeats for entire range
5. Sync "completes" with potentially thousands of unscanned blocks

---

## The Fix

### Change 1: Fetch Tree State Per Batch

```rust
// Fixed code
let prior_height = batch.range_start.saturating_sub(1);
let chain_state = if is_first_batch_in_range {
    is_first_batch_in_range = false;
    range_chain_state.clone()
} else {
    // Fetch tree state for each batch to avoid CheckpointConflict
    // with existing wallet data during incremental syncs.
    match fetch_chain_state(&client, prior_height, wallet_id).await {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(
                wallet_id = %wallet_id,
                height = %u32::from(prior_height),
                error = ?err,
                "tree state fetch failed for batch, aborting range"
            );
            range_error = true;
            break;
        }
    }
};
```

This ensures each batch receives the correct tree frontier from lightwalletd, matching what the wallet database expects.

### Change 2: Abort on Scan Failure

```rust
// Fixed error handling
Err(err) => {
    tracing::error!(
        wallet_id = %wallet_id,
        range_start = %u32::from(batch.range_start),
        limit = limit,
        error = ?err,
        "failed to scan blocks, aborting range"
    );
    range_error = true;
    break;  // Stop processing this range
}
```

When a scan fails, the range is aborted. The next sync attempt will retry from where it left off, rather than silently skipping blocks.

---

## Trade-offs

### Performance Impact

| Metric | Before | After |
|--------|--------|-------|
| Tree state fetches per range | 1 | 1 per 100 blocks |
| RPC calls for 3000-block range | 1 | ~30 |

### Why This Trade-off Is Acceptable

1. **Correctness over speed**: Missing transactions is unacceptable; slightly slower syncs are not
2. **Lightweight operation**: Tree state responses are small (~1KB protobuf)
3. **Incremental syncs are small**: Incremental syncs typically cover tens to hundreds of blocks, not thousands
4. **Alternative is complex**: Tracking tree state between batches would require deep librustzcash integration

### Future Optimization (Not Implemented)

A more sophisticated approach could cache the tree frontier after each successful scan and use it for the next batch, avoiding RPC calls entirely. This would require:

1. Extracting final tree state from `ScanSummary`
2. Constructing `ChainState` from the extracted frontier
3. Handling edge cases around scan failures and retries

This optimization is deferred as the current fix prioritizes correctness and simplicity.

---

## Verification

### Confirming the Fix Works

```bash
# Build
cargo build --release -p zkore-cli

# Sync with debug logging
RUST_LOG=debug ./target/release/zkore sync <WALLET_ID> -p <PASSWORD> 2>&1 | tee sync.log

# Check for errors (should find none after fix)
grep -i "CheckpointConflict\|failed to scan" sync.log
```

### Test Scenarios

1. **Fresh wallet sync**: Should work (was working before, still works)
2. **Incremental sync after waiting for new blocks**: Should now complete without CheckpointConflict
3. **Sync after node restart**: Should resume correctly
4. **Sync with intermittent network failures**: Should abort cleanly and retry on next sync

---

## Lessons Learned

1. **Understand library contracts**: The `ChainState` parameter's role in checkpoint management was not fully understood when the optimization was written

2. **Test incremental scenarios**: The bug only appeared in incremental syncs, which may have been under-tested compared to fresh wallet flows

3. **Fail loudly**: Silent continuation after errors ("partial scan is ok") allowed data corruption to go unnoticed; failing fast would have surfaced the issue immediately

4. **Comments can mislead**: The original comment confidently explained why empty state was correct, but the explanation was based on an incomplete understanding
