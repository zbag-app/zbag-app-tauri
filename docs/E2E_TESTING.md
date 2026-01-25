# E2E Testing with Test Bridge

This document describes the test bridge architecture and Playwright E2E testing setup for zstash.

## Overview

The test bridge is a feature-gated HTTP server that exposes Tauri IPC commands over REST endpoints. This enables end-to-end testing with Playwright against the real Rust backend without requiring the Tauri webview.

**Security note:** The test bridge is localhost-only (`127.0.0.1:19816`) and should **never** be enabled in release builds.

## Security Considerations

> **WARNING:** The test bridge is for development and CI testing only.
> - **Never use production wallets or real seed phrases**
> - Test data is ephemeral (stored in `ZSTASH_TEST_HOME`)
> - All wallet operations including `view_seed_phrase` are exposed over HTTP
> - Always use dedicated test/regtest seed phrases

## Architecture

```
Chrome Browser (localhost:1420)
  └── React Frontend (Vite dev server)
        └── VITE_TEST_BRIDGE=true
              └── HTTP fetch
                    └── Test Bridge Server (:19816)
                          └── AppState (real Rust backend)
```

The test bridge intercepts the normal Tauri IPC flow:

| Component | Port | Purpose |
|-----------|------|---------|
| Test Bridge | 19816 | HTTP server exposing Rust commands |
| Vite Dev Server | 1420 | React frontend with test transport |
| Playwright | - | Browser automation and assertions |

When `VITE_TEST_BRIDGE=true` is set, the frontend's IPC service (`src/services/ipc.ts`) routes commands via HTTP POST to `http://127.0.0.1:19816/invoke/{command}` instead of using Tauri's native IPC.

## Quick Start

Run E2E tests with a single command:

```bash
make test-e2e
```

This handles everything automatically: building the test bridge, starting it, running tests, and cleanup.

### Manual Setup (3 Terminals)

For development and debugging, you can run each component separately:

```bash
# Terminal 1: Start the Rust test bridge with isolated data directory
export ZSTASH_TEST_HOME="$(mktemp -d)"
cargo run -p zstash-app-tauri --features test-bridge

# Terminal 2: Start the Vite dev server with test bridge transport
cd apps/zstash-app-tauri
VITE_TEST_BRIDGE=true bun run dev

# Terminal 3: Run Playwright tests
cd apps/zstash-app-tauri
bunx playwright install chromium  # First time only
bun run test:e2e
```

### Playwright Options

```bash
# Run with visible browser
./scripts/e2e-test.sh --headed

# Open Playwright UI
./scripts/e2e-test.sh --ui

# Run specific test file
./scripts/e2e-test.sh tests/e2e/playwright/onboarding.spec.ts
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ZSTASH_TEST_HOME` | `mktemp -d` | Isolated data directory for test wallets. Required in test-bridge mode; empty or whitespace-only values are rejected. |
| `VITE_TEST_BRIDGE` | `false` | Enables HTTP transport in frontend |
| `VITE_TEST_BRIDGE_TIMEOUT` | `10000` | Request timeout in ms (useful for slow CI runners) |
| `ZSTASH_TEST_BRIDGE_ALLOWED_ORIGINS` | `http://localhost:1420,http://127.0.0.1:1420` | Comma-separated list of allowed browser origins for the test bridge CORS policy. |
| `ZSTASH_TEST_BRIDGE_PROBE_TIMEOUT_MS` | `15000` | Timeout in ms for the test bridge server probe to lightwalletd. |

### Test Isolation with ZSTASH_TEST_HOME

The `ZSTASH_TEST_HOME` environment variable redirects all wallet data to an isolated directory, preventing tests from polluting real user data. In test-bridge mode, it must be set to a non-empty path. When set:

- App database is created at `$ZSTASH_TEST_HOME/zstash.sqlite`
- Wallet databases are stored under `$ZSTASH_TEST_HOME/wallets/`
- Logs go to `$ZSTASH_TEST_HOME/logs/`

The `scripts/e2e-test.sh` script automatically:
1. Creates a temporary directory for `ZSTASH_TEST_HOME`
2. Cleans it up after tests complete

If `ZSTASH_TEST_HOME` is present but empty/whitespace, the script exits with an error. When `VITE_TEST_BRIDGE=true` is set, a non-empty `ZSTASH_TEST_HOME` is required.

To reset test data between manual runs:
```bash
rm -rf "$ZSTASH_TEST_HOME"
```

## Test Bridge API

The test bridge exposes commands via HTTP POST:

```
POST http://127.0.0.1:19816/invoke/{command}
Content-Type: application/json

{
  "request": {
    "schema_version": 1,
    ...command-specific fields
  }
}
```

### Health Check

```bash
curl http://127.0.0.1:19816/health
# {"status":"ok","version":"1","test_bridge":true}
```

The `test_bridge` flag confirms you're hitting the HTTP bridge.

### Example: List Wallets

```bash
curl -X POST http://127.0.0.1:19816/invoke/zstash_list_wallets \
  -H "Content-Type: application/json" \
  -d '{"request":{"schema_version":1}}'
```

### Sensitive Endpoints

The following endpoints require an explicit confirmation header:
- `zstash_view_seed_phrase`
- `zstash_restore_wallet`
- `zstash_confirm_send`

Example:

```bash
curl -X POST http://127.0.0.1:19816/invoke/zstash_view_seed_phrase \
  -H "Content-Type: application/json" \
  -H "X-Test-Bridge-Confirm: true" \
  -d '{"request":{"schema_version":1,"wallet_id":"...","reauth_token":"..."}}'
```

Calls to the seed phrase endpoint are rate-limited (1 request every 2 seconds).

### Supported Commands

All Tauri IPC commands are available via the test bridge. See `src-tauri/src/test_bridge/mod.rs` for the complete list.

## CI Workflow

The `playwright-e2e` job in `.github/workflows/ci.yml`:

1. Sets `ZSTASH_TEST_HOME` to an isolated directory
2. Builds the test bridge with `--features test-bridge`
3. Starts the test bridge in background
4. Waits for health check to pass
5. Runs Playwright tests
6. Kills the test bridge on completion

Key CI configuration:
- Tests run in single-worker mode to avoid state conflicts
- Failed tests retry up to 2 times
- Screenshots and videos are captured on failure

## Writing Tests

Tests live in `apps/zstash-app-tauri/tests/e2e/playwright/`.

### Test Structure

```typescript
import { test, expect } from '@playwright/test';

const TEST_BRIDGE_BASE_URL = 'http://127.0.0.1:19816';

test.describe('Feature Name', () => {
  test('does something', async ({ page, request }) => {
    // UI interaction
    await page.goto('/#/create');
    await expect(page.getByRole('heading', { name: 'Create Wallet' })).toBeVisible();

    // Direct API call via test bridge
    const response = await request.post(`${TEST_BRIDGE_BASE_URL}/invoke/zstash_list_wallets`, {
      data: { request: { schema_version: 1 } },
    });
    expect(response.ok()).toBeTruthy();
  });
});
```

### Best Practices

1. **Cleanup created resources** - Use `test.afterEach` to logout/cleanup wallets
2. **Use descriptive test names** - Tests should read like specifications
3. **Prefer UI interactions** - Only use direct API calls for setup/teardown or verification
4. **Check for visibility** - Use `toBeVisible()` instead of just existence checks
5. **Sequential execution** - Tests run sequentially (single worker) to avoid state conflicts

### Test Files

| File | Description |
|------|-------------|
| `onboarding.spec.ts` | Wallet creation, restore, and keystone setup flows |
| `wallet-flows.spec.ts` | Wallet lifecycle (load, unlock, lock, logout) |

## Debugging

### Interactive Debugging

Run tests with Playwright's built-in debugger:
```bash
bun run test:e2e:debug
```

This pauses before each action and opens Playwright Inspector.

### UI Mode

For visual test exploration and time-travel debugging:
```bash
bun run test:e2e:ui
```

### Viewing Test Bridge Logs

When running manually, the test bridge outputs logs to stdout. For more verbose output:
```bash
RUST_LOG=debug cargo run -p zstash-app-tauri --features test-bridge
```

In CI, test bridge logs are uploaded as artifacts on failure.

### Environment Variables for Debugging

| Variable | Description |
|----------|-------------|
| `PWDEBUG=1` | Enable Playwright debugger |
| `RUST_LOG=debug` | Verbose test bridge logging |
| `DEBUG=pw:api` | Playwright API call logging |

## Troubleshooting

### Test bridge fails to start

Check if port 19816 is already in use:
```bash
lsof -i :19816
```

Kill any existing process and retry.

### Tests timeout waiting for test bridge

The test bridge needs to compile on first run. Subsequent runs use cached builds:
```bash
# Pre-build to speed up tests
make test-bridge-build
```

If individual IPC calls time out, increase `VITE_TEST_BRIDGE_TIMEOUT`. This is separate from the Playwright web server timeout configured in `apps/zstash-app-tauri/playwright.config.ts`.

### Frontend can't connect to test bridge

Verify the test bridge is running and healthy:
```bash
curl http://127.0.0.1:19816/health
```

Ensure `VITE_TEST_BRIDGE=true` is set when starting the Vite dev server.

### CORS errors in browser console

The test bridge defaults to allowing `http://localhost:1420` and `http://127.0.0.1:1420`. If your Vite dev server uses a different origin, set `ZSTASH_TEST_BRIDGE_ALLOWED_ORIGINS` to a comma-separated list of allowed origins.

### Tests fail with "wallet not found"

Tests may be running against stale data. Reset the test home:
```bash
rm -rf "$ZSTASH_TEST_HOME"
```

Or ensure a fresh temp directory is created:
```bash
export ZSTASH_TEST_HOME="$(mktemp -d)"
```

## Make Targets

| Target | Description |
|--------|-------------|
| `make test-e2e` | Run Playwright E2E tests (starts test bridge automatically) |
| `make test-bridge` | Run the test bridge server (manual mode) |
| `make test-bridge-build` | Build the test bridge server without running |

## Differences from Production

The test bridge has minor behavioral differences from production Tauri:

1. **No event emission** - Tauri events (sync progress, balance updates) are not emitted. Tests must poll for state changes via `get_sync_progress`, `get_balance`, etc.

2. **Birthday height** - Test-created wallets skip the lightwalletd birthday height fetch and use Sapling activation height instead. This affects initial sync range but not functionality.

3. **Auto-sync disabled** - Wallets don't auto-sync on load. Tests must explicitly call `start_sync` if sync is needed.
