# Quickstart: Zkore Desktop Wallet

**Branch**: `001-zkore-desktop-wallet`
**Purpose**: Developer setup and initial implementation guide

## Prerequisites

### Required Tools

- **Rust**: 1.92.0+ (edition 2024, with rustup)
- **Bun**: 1.3.5+
- **Tauri CLI**: v2 (installed as dev dependency via `@tauri-apps/cli`, not global)

> **Note**: We use Rust 1.92.0 with edition 2024 to align with the librustzcash ecosystem. While librustzcash MSRV is 1.85.1, we target 1.92.0 for the development toolchain to leverage the latest improvements. Edition 2024 provides improved safety semantics and is production-proven in Zcash infrastructure.

### Platform-Specific

**macOS**:
```bash
xcode-select --install
```

**Linux (Ubuntu/Debian)**:
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

**Windows**:
- Visual Studio Build Tools with C++ workload
- WebView2 Runtime (usually pre-installed on Windows 11)

## Project Setup

### 1. Initialize Rust Workspace

```bash
# Create workspace root
mkdir -p crates apps tests

# Create Cargo.toml at root
cat > Cargo.toml << 'EOF'
[workspace]
resolver = "2"
members = [
    "crates/zkore-core",
    "crates/zkore-engine",
    "crates/zkore-network",
    "crates/zkore-keystone",
    "crates/zkore-tor",
    "apps/zkore-app-tauri/src-tauri",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.92.0"
license = "MIT"
repository = "https://github.com/zkore/zkore-desktop"

[workspace.dependencies]
# Zcash libraries (aligned with librustzcash/Zashi)
# Using caret constraints for semver-compatible updates
# Features enabled:
#   - orchard: Orchard shielded pool support (required)
#   - transparent-inputs: Receive transparent funds + shield them (FR-010/FR-011)
#   - pczt: PCZT signing for Keystone hardware wallet (FR-020-028)
#   - tor: Embedded Arti Tor client for fail-closed anonymization (FR-037-041)
zcash_client_backend = { version = "0.21", features = ["orchard", "transparent-inputs", "pczt", "tor"] }
zcash_client_sqlite = { version = "0.19", features = ["transparent-inputs"] }
zcash_primitives = { version = "0.26" }
zcash_protocol = { version = "0.7" }

# Modules relocated from zcash_primitives in 0.20+
zip32 = "0.2"

# Mnemonic + randomness
bip39 = "2"
rand = "0.8"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Logging
tracing-appender = "0.2"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Encoding
base64 = "0.22"

# gRPC (tonic 0.14+ required for prost 0.14 compatibility)
tonic = "0.14"
prost = "0.14"

# HTTP
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# Database
rusqlite = { version = "0.37", features = ["bundled"] }

# Error handling
thiserror = "2"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Zeroization
zeroize = { version = "1", features = ["derive"] }
secrecy = "0.8"

# UUID
uuid = { version = "1", features = ["v4", "serde"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# FFI (if needed for C bindings)
# bindgen = "0.72"
EOF
```

### 2. Create Backend Crates

```bash
# Core domain types and IPC
cargo new --lib crates/zkore-core
cargo new --lib crates/zkore-engine
cargo new --lib crates/zkore-network
cargo new --lib crates/zkore-keystone
cargo new --lib crates/zkore-tor
```

### 3. Initialize Tauri App

```bash
# Create Tauri app directory
cd apps
bun create tauri-app zkore-app-tauri --template react-ts
cd zkore-app-tauri

# Install dependencies (includes @tauri-apps/cli as dev dependency)
bun install

# Add Tauri CLI as dev dependency (preferred over global cargo install)
bun add -D @tauri-apps/cli

# Install additional UI dependencies
bun add @keystonehq/animated-qr @keystonehq/keystone-sdk
bun add qrcode.react @tanstack/react-query
bun add @radix-ui/react-dialog @radix-ui/react-dropdown-menu @radix-ui/react-tabs
bun add react-hotkeys-hook
bun add -D @types/node @axe-core/react
```

> **Note**: We use `@tauri-apps/cli` as a dev dependency rather than `cargo install tauri-cli`. This ensures consistent CLI versions across the team and integrates with bun scripts (`bun tauri dev`, `bun tauri build`).

### 4. Configure Tauri for Workspace

Edit `apps/zkore-app-tauri/src-tauri/Cargo.toml`:

```toml
[package]
name = "zkore-app-tauri"
version.workspace = true
edition.workspace = true

[dependencies]
zkore-core = { path = "../../../crates/zkore-core" }
zkore-engine = { path = "../../../crates/zkore-engine" }
zkore-network = { path = "../../../crates/zkore-network" }
zkore-keystone = { path = "../../../crates/zkore-keystone" }
zkore-tor = { path = "../../../crates/zkore-tor" }

tauri = { version = "2", features = ["devtools"] }
tauri-plugin-shell = "2"
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true

[build-dependencies]
tauri-build = { version = "2", features = [] }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

## Initial Implementation

### Step 1: zkore-core Types

Create `crates/zkore-core/src/lib.rs`:

```rust
//! Core domain types and IPC contracts for Zkore Desktop

pub mod domain;
pub mod ipc;
pub mod errors;

pub use domain::*;
pub use errors::*;
```

Create `crates/zkore-core/src/domain/mod.rs`:

```rust
//! Domain models

mod wallet;
mod account;
mod address;
mod backup;
mod balance;
mod server;
mod sync;
mod transaction;
mod transparent_utxo;

pub use wallet::*;
pub use account::*;
pub use address::*;
pub use backup::*;
pub use balance::*;
pub use server::*;
pub use sync::*;
pub use transaction::*;
pub use transparent_utxo::*;
```

### Step 2: Tauri Commands Skeleton

Create `apps/zkore-app-tauri/src-tauri/src/commands/mod.rs`:

```rust
use tauri::State;
use zkore_core::ipc::v1::*;

// Placeholder for wallet state
pub struct AppState {
    // Will hold wallet manager, sync service, etc.
}

#[tauri::command]
pub async fn zkore_create_wallet(
    state: State<'_, AppState>,
    request: CreateWalletRequest,
) -> Result<CreateWalletResponse, IpcError> {
    // Implementation in Milestone 1
    todo!()
}

#[tauri::command]
pub async fn zkore_get_balance(
    state: State<'_, AppState>,
    request: GetBalanceRequest,
) -> Result<GetBalanceResponse, IpcError> {
    // Implementation in Milestone 1
    todo!()
}

// ... more commands
```

### Step 3: Frontend IPC Client

Copy IPC types to frontend:

```bash
cp specs/001-zkore-desktop-wallet/contracts/ipc-v1.ts \
   apps/zkore-app-tauri/src/types/ipc.ts
```

Create `apps/zkore-app-tauri/src/services/ipc.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import * as IPC from '../types/ipc';

export async function createWallet(
  request: IPC.CreateWalletRequest
): Promise<IPC.CreateWalletResponse> {
  return invoke(IPC.Commands.CREATE_WALLET, { request });
}

export async function getBalance(
  request: IPC.GetBalanceRequest
): Promise<IPC.GetBalanceResponse> {
  return invoke(IPC.Commands.GET_BALANCE, { request });
}

// Event subscriptions
export function onBalanceChanged(
  callback: (event: IPC.BalanceChangedEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.BALANCE, (event) => {
    callback(event.payload as IPC.BalanceChangedEvent);
  });
}

export function onSyncProgress(
  callback: (event: IPC.SyncProgressEvent) => void
): Promise<UnlistenFn> {
  return listen(IPC.EventChannels.SYNC, (event) => {
    callback(event.payload as IPC.SyncProgressEvent);
  });
}
```

## Development Workflow

### Running Development Server

```bash
# From apps/zkore-app-tauri
bun run tauri dev
```

### Running Tests

```bash
# Rust tests
cargo test --workspace

# TypeScript tests
cd apps/zkore-app-tauri && bun test
```

### Accessibility Testing

```bash
# Run automated accessibility tests
cd apps/zkore-app-tauri && bun test:a11y

# Manual keyboard testing checklist:
# - Tab through all interactive elements
# - Enter/Space activates buttons and links
# - Escape closes modals and dropdowns
# - Arrow keys navigate within components
# - Focus indicator visible on all focused elements
```

### Building for Production

```bash
# Build release
bun run tauri build
```

## Directory Structure After Setup

```
.
├── Cargo.toml                    # Workspace manifest
├── crates/
│   ├── zkore-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── domain/
│   │       ├── ipc/
│   │       └── errors.rs
│   ├── zkore-engine/
│   ├── zkore-network/
│   ├── zkore-keystone/
│   └── zkore-tor/
├── apps/
│   └── zkore-app-tauri/
│       ├── src-tauri/
│       │   ├── Cargo.toml
│       │   ├── tauri.conf.json
│       │   └── src/
│       │       ├── main.rs
│       │       └── commands/
│       ├── src/
│       │   ├── main.tsx
│       │   ├── App.tsx
│       │   ├── components/
│       │   ├── pages/
│       │   ├── services/
│       │   └── types/
│       │       └── ipc.ts
│       └── package.json
├── tests/
│   ├── integration/
│   └── e2e/
└── specs/
    └── 001-zkore-desktop-wallet/
        ├── spec.md
        ├── plan.md
        ├── research.md
        ├── data-model.md
        ├── quickstart.md
        └── contracts/
            └── ipc-v1.ts
```

## Configuration Files

### Tauri Configuration

Edit `apps/zkore-app-tauri/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Zkore Desktop",
  "identifier": "com.zkore.desktop",
  "version": "0.1.0",
  "build": {
    "beforeDevCommand": "bun run dev",
    "beforeBuildCommand": "bun run build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Zkore Desktop",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'"
    }
  },
  "bundle": {
    "active": true,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "targets": "all"
  }
}
```

### Environment Configuration

Create `.env.development`:

```bash
# Light client server
# Mainnet: zec.rocks (regional: na.zec.rocks, eu.zec.rocks, sa.zec.rocks)
# Testnet: lwd.testnet.zec.pro (default)
ZKORE_GRPC_URL=https://lwd.testnet.zec.pro:443
ZKORE_NETWORK=testnet

# Logging
RUST_LOG=info,zkore=debug

# Log file location (logs written here automatically)
# ~/.zkore/logs/zkore.YYYY-MM-DD.log (rotated daily, 7 days retained)
```

## API Migration Notes (librustzcash 0.21+)

These notes are critical for implementation. The versions specified above differ from older tutorials/examples.

### Module Relocations

Several modules moved out of `zcash_primitives` in version 0.20+:

```rust
// OLD (0.19 and earlier):
use zcash_primitives::consensus::{BlockHeight, Network};
use zcash_primitives::memo::Memo;
use zcash_primitives::zip32::ExtendedSpendingKey;

// NEW (0.26+):
use zcash_protocol::consensus::{BlockHeight, Network};
use zcash_protocol::memo::Memo;
use zip32::ExtendedSpendingKey;  // separate crate
```

### Type Changes

```rust
// Amounts now use Zatoshis type instead of u64
use zcash_protocol::value::Zatoshis;

// OLD: builder.add_orchard_output(value: u64, recipient, memo)
// NEW: builder.add_orchard_output(value: Zatoshis, recipient, memo)
```

### Input Selection API

```rust
// OLD (0.14):
input_source.select_spendable_notes(account, anchor_height, target_amount)

// NEW (0.21+):
use zcash_client_backend::data_api::{TargetHeight, ConfirmationsPolicy};
input_source.select_spendable_notes(
    account,
    target_height: TargetHeight,
    confirmations_policy: ConfirmationsPolicy,
    target_amount
)
```

### TransparentAddressMetadata

```rust
// OLD: struct TransparentAddressMetadata { account_id, address_index }
// NEW: enum TransparentAddressMetadata { Derived { account_id, address_index }, Standalone }

// OLD: metadata.scope() returns TransparentKeyScope
// NEW: metadata.scope() returns Option<TransparentKeyScope>
```

### rusqlite 0.37

```rust
// Breaking: execute() now validates single statement only
// OLD (allowed multiple statements):
conn.execute("INSERT INTO t1 VALUES (1); INSERT INTO t2 VALUES (2);", [])?;

// NEW (must be separate calls):
conn.execute("INSERT INTO t1 VALUES (1)", [])?;
conn.execute("INSERT INTO t2 VALUES (2)", [])?;
```

## Security Practices

### Required for All Builds

1. **Commit Cargo.lock** to version control (reproducible builds)
2. **Production builds** must use `--locked` flag:
   ```bash
   cargo build --release --locked
   ```

### CI Pipeline Requirements

Add these to the CI pipeline:

```yaml
# .github/workflows/ci.yml
- name: Security audit
  run: cargo audit

- name: Build with lock verification
  run: cargo build --release --locked

- name: Clippy lints
  run: cargo clippy -- -D warnings
```

### Toolchain Pinning

Create `rust-toolchain.toml` at workspace root:

```toml
[toolchain]
channel = "1.92.0"
components = ["rustfmt", "clippy"]
```

## Next Steps

1. **Milestone 1**: Implement wallet creation, receive addresses, sync, and balance display
2. **Milestone 2**: Add send functionality, memo support, shielding
3. **Milestone 3**: Backup verification, restore with birthday
4. **Milestone 4**: Keystone integration
5. **Milestone 5**: NEAR Intents swaps
6. **Milestone 6**: Tor integration

Refer to `docs/task.md` for detailed implementation tasks per milestone.
