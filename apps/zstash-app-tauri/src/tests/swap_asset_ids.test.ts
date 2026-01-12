import { expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

const srcRoot = path.resolve(import.meta.dir, '..');

async function read(relPath: string): Promise<string> {
  return readFile(path.join(srcRoot, relPath), 'utf8');
}

test('swap-to-ZEC uses 1Click nep141 asset IDs', async () => {
  const file = await read('pages/Swap.tsx');
  expect(file).toContain('ZEC_ASSET_ID');
  expect(file).toContain('DEFAULT_NON_ZEC_ASSET_ID');
  expect(file).not.toContain('zcash:mainnet:native');
  expect(file).not.toContain('near:mainnet:native');
});

