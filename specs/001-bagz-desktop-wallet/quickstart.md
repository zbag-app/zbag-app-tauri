# Quickstart: bagZ Desktop Wallet

**Branch**: `main`
**Purpose**: Developer onboarding and build reference

## Prerequisites

### Required Tools

- **Rust**: 1.92.0 (edition 2024, with rustup)
- **Bun**: 1.3.5+
- **Tauri CLI**: v2 (installed as dev dependency via `@tauri-apps/cli`, not global)

> **Note**: bagZ pins and enforces Rust **1.92.0** for builds (via `rust-toolchain.toml` and the workspace `rust-version`) to align with librustzcash/Zashi. If this minimum changes, update `rust-toolchain.toml`, the workspace `rust-version`, and CI together.

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

### Encryption Prerequisites

- Wallet DB encryption uses **SQLCipher** via `rusqlite` `bundled-sqlcipher` (**required**). Ensure feature unification so the transitive `rusqlite` used by `zcash_client_sqlite` also builds with SQLCipher.
- KDF crate: `argon2` configured for **Argon2id**.
- AEAD crate: `chacha20poly1305` for **XChaCha20-Poly1305** wrapping.
- For DEK wrap/unwrap, bind AEAD associated data to `(wallet_id, network, aead_scheme, aead_version)` (values persisted per wallet in `wallet_encryption`).
- bagZ does not support a “system SQLCipher” build path; developers and end users do not install SQLCipher separately.

## Getting Started

### 1. Clone and Install

```bash
git clone https://github.com/bagz/bagz-desktop.git
cd bagz-desktop
make install    # Install frontend dependencies
```

### 2. Build and Run

```bash
# Development (Tauri desktop app)
make dev

# Or run CLI
make cli-run ARGS="wallet list" # CLI
```

### 3. Run Tests

```bash
make test       # All Rust library tests
make check      # Full CI validation (fmt + clippy + tests)
```

## Makefile Targets

The project includes a Makefile with shortcuts for common tasks. Run `make help` to see all available targets.

### Setup
| Target | Description |
|--------|-------------|
| `install` | Install frontend dependencies |

### Build
| Target | Description |
|--------|-------------|
| `build` | Build Rust library crates |
| `build-release` | Production build (libs) |
| `build-frontend` | Build frontend dist |

### Test
| Target | Description |
|--------|-------------|
| `test` | Run all library tests |
| `test-engine` | Test bagz-engine only |
| `test-core` | Test bagz-core only |
| `test-network` | Test bagz-network only |
| `test-keystone` | Test bagz-keystone only |
| `test-tor` | Test bagz-tor only |
| `test-migrations` | Run migration tests |

### Development
| Target | Description |
|--------|-------------|
| `dev` | Full Tauri development mode |
| `cli` | Build CLI (release) |
| `cli-run ARGS="..."` | Run CLI with arguments |

### Lint/Quality
| Target | Description |
|--------|-------------|
| `fmt` | Format Rust code |
| `fmt-check` | Check formatting (CI) |
| `clippy` | Run clippy lints |
| `clippy-strict` | Clippy with warnings as errors |
| `pre-commit` | Format + lint |
| `check` | Full CI validation |
| `audit` | Security audit |

### Clean
| Target | Description |
|--------|-------------|
| `clean` | Clean Rust build artifacts |
| `clean-frontend` | Clean frontend dist |
| `clean-all` | Clean everything |

## Directory Structure

```
.
├── Cargo.toml                    # Workspace manifest
├── Makefile                      # Build shortcuts
├── crates/
│   ├── bagz-core/               # Domain types, IPC contracts, errors
│   ├── bagz-engine/             # Wallet operations (librustzcash wrapper)
│   ├── bagz-network/            # gRPC/HTTP clients, Tor transport
│   ├── bagz-keystone/           # Hardware wallet integration (PCZT)
│   └── bagz-tor/                # Embedded Arti Tor client
├── apps/
│   ├── bagz-app-tauri/          # Tauri desktop app
│   │   ├── src-tauri/            # Rust backend (commands, state)
│   │   └── src/                  # React frontend (pages, components, services)
│   └── bagz-cli/                # Command-line interface
├── tests/
│   ├── integration/              # Integration tests
│   └── e2e/                      # End-to-end tests
└── specs/                        # Feature specifications
```

## Development Workflow

### Running Development Server

```bash
make dev        # Full Tauri development (frontend + backend)
```

This starts Vite on port 1420 with hot reload, plus the Tauri Rust backend.

### Running Tests

```bash
make test           # All Rust library tests
make test-engine    # Engine crate only
bun test            # Frontend tests (from apps/bagz-app-tauri)
```

### Pre-commit Checks

```bash
make pre-commit     # Format and lint
make check          # Full CI validation (fmt + clippy + tests)
```

### Building for Production

```bash
make build-frontend  # Build frontend first (required)
make tauri-build     # Build Tauri production app
```

## Configuration Files

### Tauri Configuration

Edit `apps/bagz-app-tauri/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "bagZ Desktop",
  "identifier": "com.bagz.desktop",
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
        "title": "bagZ Desktop",
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

> **CSP note**: The example CSP above is intentionally strict. If you see a blank window or missing assets in Tauri v2, loosen CSP to include the Tauri asset protocols (commonly `asset:` / `tauri:` and sometimes `blob:`) per the Tauri v2 docs, then tighten again.

### Environment Configuration

Create `.env.development`:

```bash
# Light client server
# Mainnet: https://lwd.zec.pro (default), https://zec.rocks (regional: https://na.zec.rocks, https://eu.zec.rocks, https://sa.zec.rocks)
# Testnet: https://lwd.testnet.zec.pro (default)
# Note: this override does NOT set wallet network. Wallet network is selected at wallet creation and is immutable.
# Note: this is for local development/CI only; release builds should rely on persisted server configuration and must not silently override user-selected servers via environment variables.
BAGZ_GRPC_URL=https://lwd.testnet.zec.pro

# Logging
RUST_LOG=info,bagz=debug

# Log file location (logs written here automatically)
# ~/.bagz/logs/bagz.YYYY-MM-DD.log (rotated daily, 7 days retained)
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

- name: Integration tests (lightwalletd matrix)
  run: |
    # Constitution Principle V: validate against at least two independent deployments.
    BAGZ_GRPC_URL=https://lwd.zec.pro cargo test --workspace
    BAGZ_GRPC_URL=https://zec.rocks cargo test --workspace

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

## Reference

- **Constitution**: [../../.specify/memory/constitution.md](../../.specify/memory/constitution.md) - Development principles
- **Spec**: [spec.md](spec.md) - Feature specification
- **Data Model**: [data-model.md](data-model.md) - Database schema
- **Tasks**: [tasks.md](tasks.md) - Implementation task breakdown
