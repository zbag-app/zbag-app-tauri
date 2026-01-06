# Bug: CheckpointConflict During Incremental Sync

## Issue Summary

Incremental wallet syncs fail with `CheckpointConflict` errors when scanning blocks. The sync continues but transactions in failed batches are **not processed**, leading to missed transactions.

## Symptoms

Log output shows repeated WARN messages:

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

## Root Cause

**File:** `crates/zkore-engine/src/sync_service.rs`
**Lines:** 544-550

```rust
// For the first batch in the range, use the pre-fetched tree state.
// For subsequent batches, use empty state - the scanner maintains
// internal state between calls so it doesn't need the tree state again.
let prior_height = batch.range_start.saturating_sub(1);
let chain_state = if is_first_batch_in_range {
    is_first_batch_in_range = false;
    range_chain_state.clone()
} else {
    empty_chain_state(prior_height)  // <-- BUG
};
```

### Why This Is Wrong

The comment assumes "the scanner maintains internal state between calls so it doesn't need the tree state again." This is **incorrect**.

While the scanner does maintain internal scanning state, librustzcash uses the `ChainState` parameter for **checkpoint creation and validation** in the wallet database. When we pass `empty_chain_state`:

1. We tell librustzcash "the commitment tree is empty at this height"
2. librustzcash tries to create a checkpoint with `tree_state: Empty`
3. But the wallet database already has tree data at that position (from previous syncs)
4. This causes a `CheckpointConflict` error

### Who Is Affected

| Wallet Type | Affected? | Reason |
|-------------|-----------|--------|
| Fresh wallet (first sync) | No | No existing checkpoints to conflict with |
| Existing wallet (incremental sync) | **Yes** | Has checkpoint data that conflicts with empty state |

## Impact

**Urgency: High (4/5)**

| Risk | Severity | Details |
|------|----------|---------|
| Missed Transactions | High | Each failed batch = 100 blocks not scanned |
| Incomplete Witnesses | Medium | Notes may exist but witnesses incomplete |
| Fund Spendability | Medium | Notes with incomplete witnesses cannot be spent |
| Sync Completion | Low | Sync "completes" but data is incomplete |

The sync does not abort on these errors - it logs WARN and continues. This means the user sees "Sync complete!" but has missing transaction data.

## The Fix

### Option A: Fetch Tree State Per Batch (Recommended)

Remove the optimization entirely. Fetch tree state for every batch.

**File:** `crates/zkore-engine/src/sync_service.rs`

**Before (lines 544-550):**
```rust
let prior_height = batch.range_start.saturating_sub(1);
let chain_state = if is_first_batch_in_range {
    is_first_batch_in_range = false;
    range_chain_state.clone()
} else {
    empty_chain_state(prior_height)
};
```

**After:**
```rust
let prior_height = batch.range_start.saturating_sub(1);
let chain_state = if is_first_batch_in_range {
    is_first_batch_in_range = false;
    range_chain_state.clone()
} else {
    // Fetch tree state for each batch to avoid CheckpointConflict
    // with existing wallet data during incremental syncs
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

**Note:** This requires `client` to be accessible in this scope. Currently it's cloned for the download task. You may need to keep a reference for the scan loop.

### Option B: Pass Continuing State Between Batches

Track the final tree state after each successful scan and pass it to the next batch.

This is more complex but avoids extra RPC calls:

```rust
// Before the batch loop
let mut current_chain_state = range_chain_state.clone();

// Inside the batch loop
let chain_state = current_chain_state.clone();

// After successful scan
if let Ok(scan_result) = scan_result {
    // Update current_chain_state from scan_result if available
    // (May require changes to how scan_cached_blocks returns data)
}
```

**Complexity:** High - requires understanding librustzcash internals to extract continuing state.

### Additional Fix: Abort on Scan Failure

Currently scan failures log WARN and continue. This should abort the range:

**File:** `crates/zkore-engine/src/sync_service.rs`
**Lines:** 581-590

**Before:**
```rust
Err(err) => {
    tracing::error!(
        wallet_id = %wallet_id,
        range_start = %u32::from(batch.range_start),
        limit = limit,
        error = ?err,
        "failed to scan blocks - transactions in this range may be missed"
    );
    // Continue - partial scan is ok, but error is logged
}
```

**After:**
```rust
Err(err) => {
    tracing::error!(
        wallet_id = %wallet_id,
        range_start = %u32::from(batch.range_start),
        limit = limit,
        error = ?err,
        "failed to scan blocks, aborting range"
    );
    range_error = true;
    break;  // Abort this range, will retry on next sync
}
```

## Testing

### Reproduce the Bug

1. Create a wallet and sync it fully
2. Wait for new blocks
3. Sync again (incremental)
4. Check logs for `CheckpointConflict` errors

### Verify the Fix

1. Apply the fix
2. Create a wallet and sync it fully
3. Wait for new blocks
4. Sync again - should complete without `CheckpointConflict` errors
5. Verify all transactions are detected

### Test Commands

```bash
# Build
cargo build --release -p zkore-cli

# Sync with debug logging
RUST_LOG=debug ./target/release/zkore sync <WALLET_ID> -p <PASSWORD> 2>&1 | tee sync.log

# Check for errors
grep -i "CheckpointConflict\|failed to scan" sync.log
```

## Files to Modify

| File | Lines | Change |
|------|-------|--------|
| `crates/zkore-engine/src/sync_service.rs` | 544-550 | Fix empty_chain_state usage |
| `crates/zkore-engine/src/sync_service.rs` | 581-590 | Abort on scan failure |

## Related Code

- `empty_chain_state()` helper: `sync_service.rs:900-903`
- `fetch_chain_state()`: `sync_service.rs:909-942`
- Tree state RPC: `crates/zkore-network/src/grpc_client.rs:328-344`
- Error code (unused): `crates/zkore-core/src/errors.rs:46` (`E4010`)

## History

- The optimization was added in commit `b703435` ("feat: major sync optimizations")
- The intent was to reduce RPC calls from 1 per batch to 1 per range
- Works for fresh syncs, breaks for incremental syncs with existing data
