import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for zbag E2E testing.
 *
 * This configuration is designed to work with the test bridge architecture:
 * 1. Test bridge server runs on port 19816 (Rust backend)
 * 2. Vite dev server runs on port 1420 (React frontend)
 * 3. Frontend uses VITE_TEST_BRIDGE=true to route IPC calls via HTTP
 *
 * Usage:
 *   1. Start the test bridge: cargo run --features test-bridge
 *   2. Start Vite dev server: VITE_TEST_BRIDGE=true bun run dev
 *   3. Run tests: bun run test:e2e
 */
export default defineConfig({
  testDir: './tests/e2e/playwright',
  fullyParallel: false, // Run tests sequentially to avoid state conflicts
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1, // Single worker to ensure sequential execution
  reporter: 'html',

  use: {
    // Base URL for the Vite dev server
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  // Web server configuration
  // Note: The test bridge (Rust) must be started separately before running tests
  webServer: {
    command: 'VITE_TEST_BRIDGE=true bun run dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
  },
});
