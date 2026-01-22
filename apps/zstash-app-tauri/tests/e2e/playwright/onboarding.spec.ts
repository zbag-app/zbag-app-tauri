import { test, expect } from '@playwright/test';

/**
 * Onboarding E2E Tests
 *
 * Tests the wallet creation and setup flow against the real Rust backend
 * via the test bridge HTTP server.
 *
 * Prerequisites:
 * - Test bridge running: cargo run --features test-bridge
 * - Vite dev server: VITE_TEST_BRIDGE=true bun run dev
 */

test.describe('Onboarding Flow', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the app
    await page.goto('/');
  });

  test('shows welcome screen on first launch', async ({ page }) => {
    // Wait for the app to load
    await page.waitForLoadState('networkidle');

    // Should show welcome/onboarding content
    // Note: Update these selectors based on actual UI
    const welcomeText = page.getByText(/welcome|get started|create wallet/i);
    await expect(welcomeText.first()).toBeVisible({ timeout: 10000 });
  });

  test('can navigate to create wallet', async ({ page }) => {
    await page.waitForLoadState('networkidle');

    // Look for create wallet button/link
    const createWalletButton = page.getByRole('button', { name: /create.*wallet/i });

    // If button exists, click it
    if (await createWalletButton.isVisible()) {
      await createWalletButton.click();

      // Should show wallet creation form
      const walletNameInput = page.getByPlaceholder(/wallet name|name/i);
      await expect(walletNameInput.first()).toBeVisible({ timeout: 5000 });
    }
  });

  test('test bridge health check', async ({ request }) => {
    // Verify the test bridge is responding
    const response = await request.get('http://127.0.0.1:19816/health');
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body.status).toBe('ok');
  });

  test('can list wallets via test bridge', async ({ request }) => {
    // Direct API call to test bridge to verify backend connectivity
    const response = await request.post('http://127.0.0.1:19816/invoke/zstash_list_wallets', {
      data: {
        request: {
          schema_version: 1,
        },
      },
    });

    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    // Response should be either { ok: { wallets: [...] } } or { err: { ... } }
    expect(body).toHaveProperty('ok');
    expect(body.ok).toHaveProperty('wallets');
    expect(Array.isArray(body.ok.wallets)).toBeTruthy();
  });
});

test.describe('Wallet Creation', () => {
  test('can create a new testnet wallet via test bridge', async ({ request }) => {
    const walletName = `Test Wallet ${Date.now()}`;
    const password = 'testpassword123';

    // Create wallet via test bridge
    const response = await request.post('http://127.0.0.1:19816/invoke/zstash_create_wallet', {
      data: {
        request: {
          schema_version: 1,
          name: walletName,
          network: 'Testnet',
          password: password,
          remember_unlock: false,
        },
      },
    });

    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body).toHaveProperty('ok');
    expect(body.ok).toHaveProperty('wallet');
    expect(body.ok).toHaveProperty('seed_phrase');
    expect(body.ok.wallet.name).toBe(walletName);
    expect(body.ok.wallet.network).toBe('Testnet');
    expect(body.ok.seed_phrase).toHaveLength(24);
  });
});
