import { test, expect } from '@playwright/test';

const TEST_BRIDGE_BASE_URL = 'http://127.0.0.1:19816';

test.describe('Test Bridge Security', () => {
  test('requires confirmation header for sensitive commands', async ({ request }) => {
    const commands = [
      'bagz_view_seed_phrase',
      'bagz_restore_wallet',
      'bagz_confirm_send',
    ];

    for (const command of commands) {
      const response = await request.post(`${TEST_BRIDGE_BASE_URL}/invoke/${command}`, {
        data: { request: {} },
      });

      expect(response.status()).toBe(403);
      const body = await response.json();
      expect(body.error).toContain('Missing confirmation header');
    }
  });

  test('rate limits view_seed_phrase', async ({ request }) => {
    const headers = { 'X-Test-Bridge-Confirm': 'true' };
    const url = `${TEST_BRIDGE_BASE_URL}/invoke/bagz_view_seed_phrase`;

    const first = await request.post(url, { data: { request: {} }, headers });
    expect(first.status()).toBe(400);

    const second = await request.post(url, { data: { request: {} }, headers });
    expect(second.status()).toBe(429);
  });
});
