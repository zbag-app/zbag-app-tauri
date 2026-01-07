# RPC Integration Testing (Testnet) — Funded Wallet Harness

This document describes how to generate a stable **testnet** wallet we can fund, and the next steps to add **high-coverage Rust integration tests** for all code paths that touch lightwalletd RPC.

## Goal

- Have a known, funded **testnet** wallet that we control (via mnemonic) so we can:
  - run end-to-end sync against real lightwalletd,
  - validate balance/transactions,
  - exercise send/broadcast paths,
  - build a repeatable integration-test harness with good coverage.

Security reminder (see `.specify/memory/constitution.md`): do not log or persist secrets outside the Rust trust boundary. Never paste mnemonics into issues/PRs.

---

## 1) Print the funded test wallet address (utest…)

From repo root:

```sh
cargo run -p zkore-engine --bin zkore-it-wallet -- --network testnet
```

This uses (and will create if missing) a local mnemonic file at:

- `~/.zkore/dev/it-wallet.mnemonic`

The **current** derived address (account `0`, shielded-only UA) is:

- `utest1sq77d3wuc47qa4xr6v37pn3q4rmtxlxr3csmx3ny80jhtfys3q7gh4598clsngpvnhmza3wge9fqppknztnf6l9ndt8tvtejasecazqm`

If you delete/overwrite the mnemonic file, the address will change.

### Optional: show the mnemonic (keep private)

```sh
cargo run -p zkore-engine --bin zkore-it-wallet -- --network testnet --print-mnemonic
```

### Regenerate (rotates address)

```sh
cargo run -p zkore-engine --bin zkore-it-wallet -- --network testnet --force-new
```

---

## 2) Fund the wallet

- Send testnet ZEC to the `utest1...` address above.
- Reply in the active thread with:
  - txid
  - approximate amount
  - (optional) how many confirmations you waited for

Once funded, we can run integration tests that require spendable notes/UTXOs.

---

## 3) Confirm RPC connectivity (lightwalletd)

Known-good testnet lightwalletd endpoint:

- `lwd.testnet.zec.pro:443` (TLS)

Quick probe with `grpcurl`:

```sh
grpcurl -d '{}' lwd.testnet.zec.pro:443 cash.z.wallet.sdk.rpc.CompactTxStreamer/GetLightdInfo
```

Note: the app uses `https://lwd.testnet.zec.pro` as the configured gRPC URL; TLS must be enabled for `https://...`.

---

## 4) Debug logging in dev (Tauri)

Run the app:

```sh
cd apps/zkore-app-tauri
bun run tauri dev
```

Logging notes:
- In debug builds, Rust logs are printed to stderr (so they appear in the `tauri dev` terminal).
- You can override verbosity with `RUST_LOG`, for example:

```sh
export RUST_LOG='zkore_engine=debug,zkore_network=debug,zkore_tor=debug,zkore_app_tauri_lib=debug'
export RUST_BACKTRACE=full
```

---

## 5) Next steps (integration test plan)

### Phase A — “Probe” tests (no funds required)

- Keep/extend the existing gated network probe tests in `crates/zkore-network/tests/`.
- Ensure probes cover:
  - TLS + connect (`GrpcClient::connect`)
  - basic RPC call (`GetLightdInfo`)
  - (optionally) a small range scan against tip.

These should remain **CI-gated** (only run when `CI=1` and `ZKORE_GRPC_URL` is set) to avoid flakiness locally/CI.

### Phase B — Funded wallet tests (requires mnemonic + funds)

Add a separate test suite gated behind explicit env vars, e.g.:
- `ZKORE_IT_MNEMONIC` (or read `~/.zkore/dev/it-wallet.mnemonic` locally)
- `ZKORE_GRPC_URL=https://lwd.testnet.zec.pro`

Planned coverage (Rust, integration-level):
- **Sync**:
  - start sync, wait for completion, assert wallet status transitions are sane
  - scan blocks and detect received transactions for the funded wallet
- **Balance**:
  - verify non-zero balance after funding
  - verify balance changes after spends
- **Send**:
  - prepare/send small amount to a fresh testnet UA (or back to self)
  - broadcast and confirm it appears in subsequent sync
- **Error paths**:
  - invalid endpoint, TLS misconfig, timeout handling (should not panic)
  - tor-enabled fail-closed behavior (if applicable)

Important constraints:
- Tests that spend funds must be explicitly opted-in (env gated and/or `#[ignore]`), since they mutate wallet state.
- Never print mnemonics/spending keys in test logs. Use redacted logs only.

### Phase C — Tooling polish

- Add small CLI helpers (if needed) to:
  - print wallet address (done: `zkore-it-wallet`)
  - probe endpoint
  - query balance / last sync height
  - generate a recipient testnet UA

---

## References

- CLI tool source: `crates/zkore-engine/src/bin/zkore-it-wallet.rs`
- Security rules: `.specify/memory/constitution.md`
- Network crate tests: `crates/zkore-network/tests/`

