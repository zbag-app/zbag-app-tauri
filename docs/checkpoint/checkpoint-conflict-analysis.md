# CheckpointConflict Analysis and Resolution

## Problem Statement

During incremental wallet syncs, the sync service encounters `CheckpointConflict` errors:

```
WARN zkore_engine::sync_service: failed to scan blocks
  error=Wallet(CommitmentTree(Storage(CheckpointConflict {
    checkpoint_id: BlockHeight(461003),
    checkpoint: Checkpoint { tree_state: Empty, marks_removed: {} },
    extant_tree_state: AtPosition(Position(61450)),
    extant_marks_removed: None
  })))
```

The sync reports completion but transactions in failed batches are silently missed, leading to:

- Incorrect balance display
- Unspendable funds (missing witnesses)
- False "sync complete" status

---

## Technical Background

### Zcash Commitment Trees

Zcash shielded transactions use cryptographic commitment trees (Sapling and Orchard). When scanning blocks:

1. Each shielded output adds a commitment to the tree
2. The tree's "frontier" (rightmost path) computes witnesses
3. Witnesses prove note ownership at spending time
4. Without valid witnesses, notes cannot be spent

### The ChainState Contract

`scan_cached_blocks()` from librustzcash requires a `ChainState` parameter containing:

- Block height and hash
- Sapling commitment tree frontier
- Orchard commitment tree frontier

This state serves two purposes:

1. **Witness computation** - scanner needs tree frontiers for received notes
2. **Checkpoint validation** - librustzcash creates checkpoints that must be consistent with existing wallet data

---

## Root Cause

### The Flawed Pattern

The original sync code fetched tree state once per range, then used empty state for subsequent batches:

```rust
// WRONG - causes CheckpointConflict on incremental syncs
let chain_state = if is_first_batch_in_range {
    range_chain_state.clone()  // Correct for first batch
} else {
    empty_chain_state(prior_height)  // BUG: Empty for subsequent batches
};
```

### Why This Fails

| Scenario | Has Existing Tree Data? | Empty State Valid? |
|----------|------------------------|-------------------|
| Fresh sync (new wallet) | No | Yes |
| Incremental sync | Yes | **No** - conflicts with stored checkpoints |

When we pass `empty_chain_state`:

1. We declare "the commitment tree is empty at height N"
2. librustzcash attempts to create a checkpoint with `tree_state: Empty`
3. The wallet database already has tree data at a different position
4. **Conflict**: empty vs. non-empty tree state at the same checkpoint

---

## How Zashi Solves This

### iOS SDK (BlockScanner.swift)

```swift
repeat {
    let startHeight = previousScannedHeight + 1

    // CRITICAL: Fetch TreeState from lightwalletd for EVERY batch
    let fromState = try await service.getTreeState(BlockID(height: startHeight - 1))

    scanSummary = try await self.rustBackend.scanBlocks(
        fromHeight: Int32(startHeight),
        fromState: fromState,  // Always fresh from server
        limit: batchSize
    )
} while !Task.isCancelled && scannedNewBlocks
```

### Android SDK (CompactBlockProcessor.kt)

```kotlin
val fromState = fetchTreeStateForHeight(
    height = batch.range.start - 1,  // Block BEFORE scan range
    downloader = downloader
) ?: return SyncingResult.DownloadFailed(...)

backend.scanBlocks(batch.range.start, fromState, batch.range.length())
```

### Key Insight

Zashi fetches a **fresh TreeState from lightwalletd for every batch scan**. They do not cache or reuse tree state between batches.

---

## Current Zkore Implementation Status

The current `sync_service.rs` correctly fetches tree state per batch:

```rust
// Lines 544-561 - CORRECT
let chain_state = if is_first_batch_in_range {
    is_first_batch_in_range = false;
    range_chain_state.clone()
} else {
    // Fetch tree state for each batch to avoid CheckpointConflict
    match fetch_chain_state(&client, prior_height, wallet_id).await {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(..., "tree state fetch failed for batch, aborting range");
            range_error = true;
            break;
        }
    }
};
```

This matches Zashi's approach and is correct.

---

## Outstanding Issue: GetBlockRange Off-by-One

### The Bug

`GetBlockRange(start, end)` in lightwalletd uses **inclusive** bounds, but Rust `Range` uses **exclusive** end bounds.

Current code passes the exclusive end directly:

```rust
// sync_service.rs lines 956-965 - POTENTIAL BUG
async fn download_blocks_with_retry(
    client: &...,
    start: BlockHeight,
    end: BlockHeight,  // This is exclusive from the range
    max_retries: u32,
) -> ... {
    match client.get_block_range(start, end).await {  // Passed directly - wrong!
```

### The Fix

The harness-plan worktree has the correct fix:

```rust
// CORRECT - from harness-plan
async fn download_blocks_with_retry(
    client: &...,
    start: BlockHeight,
    end_exclusive: BlockHeight,
    max_retries: u32,
) -> anyhow::Result<Vec<CompactBlock>> {
    if end_exclusive <= start {
        return Ok(vec![]);
    }
    let end_inclusive = end_exclusive.saturating_sub(1);

    match client.get_block_range(start, end_inclusive).await {
```

### Impact

Without this fix:

- Each batch may download one extra block
- Blocks at batch boundaries may be downloaded twice
- Potential for subtle scanning inconsistencies

---

## Required Changes

### Change 1: Fix GetBlockRange Off-by-One

In `sync_service.rs`, modify `download_blocks_with_retry`:

```rust
/// Download blocks with retry and exponential backoff.
async fn download_blocks_with_retry(
    client: &zkore_network::grpc_client::GrpcClient,
    start: BlockHeight,
    end_exclusive: BlockHeight,  // Renamed for clarity
    max_retries: u32,
) -> anyhow::Result<Vec<CompactBlock>> {
    // GetBlockRange expects inclusive end, but we receive exclusive from scan ranges
    if end_exclusive <= start {
        return Ok(vec![]);
    }
    let end_inclusive = end_exclusive.saturating_sub(1);

    let mut attempt = 0;
    loop {
        match client.get_block_range(start, end_inclusive).await {
            // ... rest unchanged
        }
    }
}
```

### Change 2: Verify Error Handling (Already Correct)

The current implementation correctly aborts on scan failure:

```rust
Err(err) => {
    tracing::error!(..., "failed to scan blocks, aborting range");
    range_error = true;
    break;  // Stop processing - do not silently continue
}
```

This is correct. Never log-and-continue on scan failures.

---

## Verification Checklist

After applying the fix:

- [ ] Fresh wallet sync completes without errors
- [ ] Incremental sync (after waiting for new blocks) completes without CheckpointConflict
- [ ] No duplicate block downloads at batch boundaries
- [ ] Scan failures abort the range (not silently continue)
- [ ] Self-send transaction succeeds after sync

### Test Command

```bash
ZKORE_GRPC_URL=https://lwd.testnet.zec.pro \
  cargo run -p zkore-harness -- sync-shield-send \
  --root sessions/testnet-harness-wallet \
  --wallet-id <WALLET_ID> \
  --password-stdin \
  --recipient-ua <WALLET_UA> \
  --amount-zat 10000 \
  --timeout-secs 900 \
  --reset-sync-state <<< "pw"
```

Expected: No `CheckpointConflict` warnings, transaction succeeds.

---

## Performance Considerations

### Trade-off: More RPC Calls

| Metric | Before (once per range) | After (per batch) |
|--------|------------------------|-------------------|
| Tree state fetches for 3000 blocks | 1 | ~30 |
| RPC overhead | Minimal | ~30KB total |

### Why Acceptable

1. **Correctness over speed** - missing transactions is unacceptable
2. **Lightweight operation** - TreeState responses are ~1KB protobuf
3. **Incremental syncs are small** - typically tens to hundreds of blocks
4. **Matches Zashi** - battle-tested approach

### Future Optimization (Not Implemented)

Cache tree frontier from each `ScanSummary` result and construct `ChainState` for next batch without RPC. This requires:

1. Extracting final tree state from scan result
2. Constructing `ChainState` from extracted frontier
3. Handling edge cases around failures and retries

Deferred - current approach prioritizes correctness.

---

## References

### Zkore Internal References

| File | Description |
|------|-------------|
| `../../crates/zkore-engine/src/sync_service.rs` | Main sync implementation (this worktree) |
| `../../../Zkore-sync_service-wt-testnet-harness-plan/crates/zkore-engine/src/sync_service.rs` | Harness-plan worktree with off-by-one fix |
| `../../../Zkore-sync_service-wt-testnet-harness-plan/CHECKPOINT_CONFLICT_RESOLUTION.md` | Original issue documentation |

### Zashi Reference Implementation

Paths relative to `../../../../zashi/`:

| File | Description |
|------|-------------|
| `zcash-swift-wallet-sdk/Sources/ZcashLightClientKit/Block/Scan/BlockScanner.swift` | iOS scanning logic (lines 47-67) |
| `zcash-android-wallet-sdk/sdk-lib/src/main/java/cash/z/ecc/android/sdk/block/processor/CompactBlockProcessor.kt` | Android scanning logic (lines 1883-1990) |
| `zcash-light-client-ffi/rust/src/lib.rs` | Rust FFI for `zcashlc_scan_blocks` (lines 1692-1726) |
| `zingolib/pepper-sync/src/scan/compact_blocks.rs` | Alternative scanning approach (no librustzcash scanner) |

### Zcash Documentation

Paths relative to `../../../../zashi/zcash-docs/`:

| File | Description |
|------|-------------|
| `source/rtd_pages/ux_wallet_checklist.rst` | Zcash Feature UX Checklist |
| `source/rtd_pages/wallet_threat_model.md` | Wallet security threat model |
| `source/rtd_pages/privacy_recommendations_best_practices.rst` | Privacy best practices |

### librustzcash API

| Crate | Function |
|-------|----------|
| `zcash_client_backend::data_api::chain` | `scan_cached_blocks()` |
| `zcash_client_backend::data_api::chain` | `ChainState` |
| `zcash_client_sqlite` | `WalletDb`, `FsBlockDb` |

### lightwalletd gRPC

| RPC | Description |
|-----|-------------|
| `GetTreeState(height)` | Returns TreeState with Sapling/Orchard frontiers |
| `GetBlockRange(start, end)` | Returns compact blocks (inclusive bounds) |
| `GetLatestBlock()` | Returns current chain tip |
