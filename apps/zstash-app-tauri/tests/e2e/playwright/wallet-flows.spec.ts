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

test.describe('Integration flows (test bridge)', () => {
  test('create wallet → backup verify → receive address', async ({ request }) => {
    const walletName = `Flow Wallet ${Date.now()}`;
    const password = 'testpassword123';

    const create = unwrapOk(
      await invoke<{
        wallet: { id: string };
        seed_phrase: string[];
      }>(request, 'zstash_create_wallet', {
        schema_version: 1,
        name: walletName,
        network: 'Testnet',
        password,
        remember_unlock: false,
      })
    );

    expect(create.seed_phrase).toHaveLength(24);

    const challenge = unwrapOk(
      await invoke<{
        challenge: { challenge_id: string; indices: number[] };
      }>(request, 'zstash_get_backup_challenge', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );

    const wordChallenges: Record<string, string> = {};
    for (const index of challenge.challenge.indices) {
      wordChallenges[String(index)] = create.seed_phrase[index - 1];
    }

    const verify = unwrapOk(
      await invoke<{ verified: boolean }>(request, 'zstash_verify_backup', {
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
        'zstash_get_receive_address',
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
      }>(request, 'zstash_create_wallet', {
        schema_version: 1,
        name: walletName,
        network: 'Testnet',
        password,
        remember_unlock: false,
      })
    );

    const locked = unwrapOk(
      await invoke<{ locked: boolean }>(request, 'zstash_lock_wallet', {
        schema_version: 1,
        wallet_id: create.wallet.id,
      })
    );

    expect(locked.locked).toBe(true);

    const unlocked = unwrapOk(
      await invoke<{ unlocked: boolean }>(request, 'zstash_unlock_wallet', {
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
      }>(request, 'zstash_get_balance', {
        schema_version: 1,
        account_id: 0,
      })
    );

    expect(balance.balance).toHaveProperty('total');
    expect(typeof balance.balance.total).toBe('string');
  });
});
