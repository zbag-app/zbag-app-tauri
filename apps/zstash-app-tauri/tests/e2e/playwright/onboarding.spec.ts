import { test, expect, type Page, type APIRequestContext } from '@playwright/test';

const TEST_BRIDGE_BASE_URL = 'http://127.0.0.1:19816';

// Track created wallet IDs for cleanup
let createdWalletId: string | null = null;

test.afterEach(async ({ request }) => {
  if (createdWalletId) {
    try {
      // Logout the wallet to release resources (no delete command available)
      await request.post(`${TEST_BRIDGE_BASE_URL}/invoke/zstash_logout_wallet`, {
        data: {
          request: {
            schema_version: 1,
            wallet_id: createdWalletId,
          },
        },
      });
    } catch {
      // Ignore cleanup errors
    }
    createdWalletId = null;
  }
});

async function gotoCreateWallet(page: Page) {
  await page.goto('/#/create');
  await expect(page.getByRole('heading', { name: 'Create Wallet' })).toBeVisible();
}

test.describe('Onboarding UI', () => {
  test('shows create wallet form', async ({ page }) => {
    await gotoCreateWallet(page);

    await expect(page.getByLabel('Network')).toBeVisible();
    await expect(page.getByLabel('Wallet name')).toBeVisible();
    await expect(page.getByLabel('Password', { exact: true })).toBeVisible();
    await expect(page.getByLabel('Confirm password')).toBeVisible();
    await expect(page.getByRole('button', { name: /create wallet/i })).toBeVisible();
  });

  test('validates required fields', async ({ page }) => {
    await gotoCreateWallet(page);

    await page.getByRole('button', { name: /create wallet/i }).click();
    await expect(page.getByText('Wallet name is required.')).toBeVisible();
  });

  test('validates password confirmation', async ({ page }) => {
    await gotoCreateWallet(page);

    await page.getByLabel('Wallet name').fill('Mismatch Wallet');
    await page.getByLabel('Password', { exact: true }).fill('password123');
    await page.getByLabel('Confirm password').fill('password456');
    await page.getByRole('button', { name: /create wallet/i }).click();

    await expect(page.getByText('Passwords do not match.')).toBeVisible();
  });

  test('navigates to restore flow', async ({ page }) => {
    await gotoCreateWallet(page);

    await page.getByRole('link', { name: /restore from seed phrase/i }).click();
    await expect(page.getByRole('heading', { name: 'Restore Wallet' })).toBeVisible();
  });

  test('navigates to keystone setup', async ({ page }) => {
    await gotoCreateWallet(page);

    await page.getByRole('link', { name: /connect hardware wallet/i }).click();
    await expect(page.getByRole('heading', { name: 'Connect Keystone' })).toBeVisible();
  });
});

test.describe('Test Bridge API', () => {
  test('health check', async ({ request }) => {
    const response = await request.get(`${TEST_BRIDGE_BASE_URL}/health`);
    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body.status).toBe('ok');
  });

  test('can list wallets via test bridge', async ({ request }) => {
    const response = await request.post(`${TEST_BRIDGE_BASE_URL}/invoke/zstash_list_wallets`, {
      data: {
        request: {
          schema_version: 1,
        },
      },
    });

    expect(response.ok()).toBeTruthy();

    const body = await response.json();
    expect(body).toHaveProperty('ok');
    expect(body.ok).toHaveProperty('wallets');
    expect(Array.isArray(body.ok.wallets)).toBeTruthy();
  });

  test('can create a new testnet wallet via test bridge', async ({ request }) => {
    const walletName = `Test Wallet ${Date.now()}`;
    const password = 'testpassword123';

    const response = await request.post(`${TEST_BRIDGE_BASE_URL}/invoke/zstash_create_wallet`, {
      data: {
        request: {
          schema_version: 1,
          name: walletName,
          network: 'Testnet',
          password,
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

    // Track for cleanup
    createdWalletId = body.ok.wallet.id;
  });
});
