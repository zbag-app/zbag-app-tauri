/**
 * Wallet Flows E2E Tests
 *
 * ## Test Bridge Limitations
 *
 * When running against the test bridge (not full Tauri):
 *
 * 1. **Sync Events**: The test bridge passes `None` for event handlers in `start_sync`.
 *    Tests cannot receive real-time sync progress events. Use `get_sync_progress` polling
 *    instead to check sync status.
 *
 * 2. **Birthday Height**: Newly created wallets use Sapling activation height as birthday,
 *    not the current chain tip. This means initial sync may take longer than expected.
 *
 * 3. **Auto-sync Disabled**: Wallets do not auto-sync on load. Tests must explicitly call
 *    `start_sync` and poll `get_sync_progress` to monitor progress.
 */
import { test, expect, type APIRequestContext } from '@playwright/test';

const TEST_BRIDGE_BASE_URL = 'http://127.0.0.1:19816';

type IpcError = { code: string; message: string };

type IpcResult<T> =
  | { ok: T }
  | { err: IpcError };

function unwrapOk<T>(result: IpcResult<T>): T {
  if ('err' in result) {
    throw new Error(`IPC error ${result.err.code}: ${result.err.message}`);
  }
  return result.ok;
}

async function invoke<T>(
  request: APIRequestContext,
  command: string,
  payload: Record<string, unknown>
): Promise<IpcResult<T>> {
  const response = await request.post(`${TEST_BRIDGE_BASE_URL}/invoke/${command}`, {
    data: { request: payload },
  });
  expect(response.ok()).toBeTruthy();
  return response.json();
}

// Track created wallet IDs for cleanup
let createdWalletId: string | null = null;

test.afterEach(async ({ request }) => {
  if (createdWalletId) {
    const walletIdToCleanup = createdWalletId;
    createdWalletId = null;

    try {
      await invoke(request, 'zbag_stop_sync', {
        schema_version: 1,
        wallet_id: walletIdToCleanup,
      });
    } catch (e) {
      // Only warn on unexpected errors (sync not running is expected)
      const msg = e instanceof Error ? e.message : String(e);
      if (!msg.includes('SYNC_NOT_RUNNING')) {
        console.warn(`[cleanup] stop_sync failed for ${walletIdToCleanup}: ${msg}`);
      }
    }

    try {
      await invoke(request, 'zbag_logout_wallet', {
        schema_version: 1,
        wallet_id: walletIdToCleanup,
      });
    } catch (e) {
      console.warn(`[cleanup] logout_wallet failed for ${walletIdToCleanup}: ${e instanceof Error ? e.message : e}`);
    }
  }
});

test.describe('Integration flows (test bridge)', () => {
  test('create wallet → backup verify → receive address', async ({ request }) => {
    const walletName = `Flow Wallet ${Date.now()}`;
    const password = 'testpassword123';

    const create = unwrapOk(
      await invoke<{
        wallet: { id: string };
        seed_phrase: string[];
      }>(request, 'zbag_create_wallet', {
        schema_version: 1,
        name: walletName,
        network: 'Testnet',
        password,
        remember_unlock: false,
      })
    );

    // Track for cleanup
    createdWalletId = create.wallet.id;

    expect(create.seed_phrase).toHaveLength(24);

    const challenge = unwrapOk(
      await invoke<{
        challenge: { challenge_id: string; indices: number[] };
      }>(request, 'zbag_get_backup_challenge', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );

    const wordChallenges: Record<string, string> = {};
    for (const index of challenge.challenge.indices) {
      wordChallenges[String(index)] = create.seed_phrase[index - 1];
    }

    const verify = unwrapOk(
      await invoke<{ verified: boolean }>(request, 'zbag_verify_backup', {
        schema_version: 1,
        wallet_id: create.wallet.id,
        challenge_id: challenge.challenge.challenge_id,
        word_challenges: wordChallenges,
      })
    );

    expect(verify.verified).toBe(true);

    const address = unwrapOk(
      await invoke<{ address: { encoded: string; address_type: string } }>(
        request,
        'zbag_get_receive_address',
        {
          schema_version: 1,
          account_id: 0,
          address_type: 'ShieldedOnly',
        }
      )
    );

    expect(address.address.encoded).toBeTruthy();
    expect(address.address.address_type).toBe('ShieldedOnly');
  });

  test('lock/unlock cycle → balance', async ({ request }) => {
    const walletName = `Lock Flow ${Date.now()}`;
    const password = 'testpassword123';

    const create = unwrapOk(
      await invoke<{
        wallet: { id: string };
      }>(request, 'zbag_create_wallet', {
        schema_version: 1,
        name: walletName,
        network: 'Testnet',
        password,
        remember_unlock: false,
      })
    );

    // Track for cleanup
    createdWalletId = create.wallet.id;

    const locked = unwrapOk(
      await invoke<{ locked: boolean }>(request, 'zbag_lock_wallet', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );

    expect(locked.locked).toBe(true);

    const unlocked = unwrapOk(
      await invoke<{ unlocked: boolean }>(request, 'zbag_unlock_wallet', {
        schema_version: 1,
        wallet_id: create.wallet.id,
        password,
        remember_unlock: false,
      })
    );

    expect(unlocked.unlocked).toBe(true);

    const balance = unwrapOk(
      await invoke<{
        balance: {
          shielded_spendable: string;
          shielded_pending: string;
          transparent_total: string;
          total: string;
        };
      }>(request, 'zbag_get_balance', {
        schema_version: 1,
        account_id: 0,
      })
    );

    expect(balance.balance).toHaveProperty('total');
    expect(typeof balance.balance.total).toBe('string');
  });

  test('shield_funds requires reauth and handles no transparent funds', async ({ request }) => {
    const walletName = `Shield Test ${Date.now()}`;
    const password = 'testpassword123';

    // Create wallet
    const create = unwrapOk(
      await invoke<{ wallet: { id: string } }>(request, 'zbag_create_wallet', {
        schema_version: 1,
        name: walletName,
        network: 'Testnet',
        password,
        remember_unlock: false,
      })
    );
    createdWalletId = create.wallet.id;

    // Get reauth token for Spend purpose
    const reauth = unwrapOk(
      await invoke<{ reauth_token: string; expires_at: number }>(
        request,
        'zbag_reauth_wallet',
        {
          schema_version: 1,
          wallet_id: create.wallet.id,
          password,
          purpose: 'Spend',
        }
      )
    );
    expect(reauth.reauth_token).toBeTruthy();

    // Shield funds - will fail (no transparent balance)
    const result = await invoke<{ txid: string }>(request, 'zbag_shield_funds', {
      schema_version: 1,
      account_id: 0,
      consolidate: false,
      reauth_token: reauth.reauth_token,
    });

    expect('err' in result).toBe(true);
    if ('err' in result) {
      expect(result.err.code).toBeTruthy();
    }
  });

  test('sync polling workflow: start, poll progress, stop', async ({ request }) => {
    const walletName = `Sync Poll Test ${Date.now()}`;
    const password = 'testpassword123';

    const create = unwrapOk(
      await invoke<{ wallet: { id: string } }>(request, 'zbag_create_wallet', {
        schema_version: 1,
        name: walletName,
        network: 'Testnet',
        password,
        remember_unlock: false,
      })
    );
    createdWalletId = create.wallet.id;

    // Start sync
    const startResult = unwrapOk(
      await invoke<{ started: boolean }>(request, 'zbag_start_sync', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );
    expect(startResult.started).toBe(true);

    // Poll progress (demonstrates documented pattern)
    const progress = unwrapOk(
      await invoke<{
        progress: { phase: string; progress_percent: number };
      }>(request, 'zbag_get_sync_progress', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );
    expect(progress.progress).toHaveProperty('phase');
    expect(typeof progress.progress.progress_percent).toBe('number');

    // Stop sync
    const stopResult = unwrapOk(
      await invoke<{ stopped: boolean }>(request, 'zbag_stop_sync', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );
    expect(stopResult.stopped).toBe(true);

    // Verify idle
    const afterStop = unwrapOk(
      await invoke<{ progress: { phase: string } }>(request, 'zbag_get_sync_progress', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );
    expect(afterStop.progress.phase).toBe('Idle');
  });
});

test.describe('Error handling (test bridge)', () => {
  test('unlock with wrong password returns error', async ({ request }) => {
    const walletName = `Error Test Wallet ${Date.now()}`;
    const password = 'correctpassword123';
    const wrongPassword = 'wrongpassword456';

    // Create a wallet
    const create = unwrapOk(
      await invoke<{
        wallet: { id: string };
      }>(request, 'zbag_create_wallet', {
        schema_version: 1,
        name: walletName,
        network: 'Testnet',
        password,
        remember_unlock: false,
      })
    );

    // Track for cleanup
    createdWalletId = create.wallet.id;

    // Lock the wallet
    const locked = unwrapOk(
      await invoke<{ locked: boolean }>(request, 'zbag_lock_wallet', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );
    expect(locked.locked).toBe(true);

    // Try to unlock with wrong password
    const result = await invoke<{ unlocked: boolean }>(request, 'zbag_unlock_wallet', {
      schema_version: 1,
      wallet_id: create.wallet.id,
      password: wrongPassword,
      remember_unlock: false,
    });

    expect('err' in result).toBe(true);
    if ('err' in result) {
      expect(result.err.code).toBeTruthy();
    }
  });

  test('get balance for non-existent account returns error', async ({ request }) => {
    const walletName = `Balance Error Test ${Date.now()}`;
    const password = 'testpassword123';

    // Create a wallet
    const create = unwrapOk(
      await invoke<{
        wallet: { id: string };
      }>(request, 'zbag_create_wallet', {
        schema_version: 1,
        name: walletName,
        network: 'Testnet',
        password,
        remember_unlock: false,
      })
    );

    // Track for cleanup
    createdWalletId = create.wallet.id;

    // Try to get balance for non-existent account (high index)
    const result = await invoke<{
      balance: { total: string };
    }>(request, 'zbag_get_balance', {
      schema_version: 1,
      account_id: 999,
    });

    expect('err' in result).toBe(true);
  });

  test('unknown command returns 404', async ({ request }) => {
    const response = await request.post(`${TEST_BRIDGE_BASE_URL}/invoke/zbag_nonexistent`, {
      data: { request: { schema_version: 1 } },
    });

    expect(response.status()).toBe(404);
  });
});
