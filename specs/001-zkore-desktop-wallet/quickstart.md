# Quickstart: Zkore Desktop Wallet

**Branch**: `001-zkore-desktop-wallet`
**Purpose**: Developer setup and initial implementation guide

## Prerequisites

### Required Tools

- **Rust**: 1.75+ (with rustup)
- **Node.js**: 20+ (LTS)
- **pnpm**: 8+ (package manager)
- **Tauri CLI**: v2

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
edition = "2021"
license = "MIT"
repository = "https://github.com/zkore/zkore-desktop"

[workspace.dependencies]
# Zcash libraries
zcash_client_backend = { version = "0.14", features = ["orchard", "pczt", "tor"] }
zcash_client_sqlite = { version = "0.12" }
zcash_primitives = { version = "0.19" }
zcash_protocol = { version = "0.4" }

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# gRPC
tonic = "0.12"
prost = "0.13"

# HTTP
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }

# Database
rusqlite = { version = "0.32", features = ["bundled"] }

# Error handling
thiserror = "1"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Zeroization
zeroize = { version = "1", features = ["derive"] }

# UUID
uuid = { version = "1", features = ["v4", "serde"] }

# Time
chrono = { version = "0.4", features = ["serde"] }
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
# Install Tauri CLI
cargo install tauri-cli --version "^2.0.0"

# Create Tauri app
cd apps
pnpm create tauri-app zkore-app-tauri --template react-ts
cd zkore-app-tauri

# Install dependencies
pnpm install

# Install additional UI dependencies
pnpm add @keystonehq/animated-qr @keystonehq/keystone-sdk
pnpm add qrcode.react @tanstack/react-query
pnpm add -D @types/node
```

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
mod balance;
mod transaction;

pub use wallet::*;
pub use balance::*;
pub use transaction::*;
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
pnpm tauri dev
```

### Running Tests

```bash
# Rust tests
cargo test --workspace

# TypeScript tests
cd apps/zkore-app-tauri && pnpm test
```

### Building for Production

```bash
# Build release
pnpm tauri build
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
    "beforeDevCommand": "pnpm dev",
    "beforeBuildCommand": "pnpm build",
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
ZKORE_GRPC_URL=https://zec.rocks:443
ZKORE_NETWORK=testnet

# Logging
RUST_LOG=info,zkore=debug
```

## Next Steps

1. **Milestone 1**: Implement wallet creation, receive addresses, sync, and balance display
2. **Milestone 2**: Add send functionality, memo support, shielding
3. **Milestone 3**: Backup verification, restore with birthday
4. **Milestone 4**: Keystone integration
5. **Milestone 5**: NEAR Intents swaps
6. **Milestone 6**: Tor integration

Refer to `docs/task.md` for detailed implementation tasks per milestone.
