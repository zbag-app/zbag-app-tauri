import { expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

const srcRoot = path.resolve(import.meta.dir, '..');

async function read(relPath: string): Promise<string> {
  return readFile(path.join(srcRoot, relPath), 'utf8');
}

test('swap-from-ZEC sends input_amount in ZEC units (not zatoshis)', async () => {
  const file = await read('pages/SwapFromZec.tsx');

  // The IPC contract expects asset-denominated amounts. The Rust backend handles conversion
  // to smallest units (zatoshis) for the 1Click API. Passing zatoshis here would be converted
  // again and result in incorrect (too-large) quotes.
  expect(file).toContain("swap_type: 'FromZec'");
  expect(file).toContain('input_amount: inputAmountZecTrimmed');
  expect(file).not.toContain('input_amount: inputAmountZatoshis');
});

