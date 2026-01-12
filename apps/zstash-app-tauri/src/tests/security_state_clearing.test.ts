import { expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

const srcRoot = path.resolve(import.meta.dir, '..');

async function read(relPath: string): Promise<string> {
  return readFile(path.join(srcRoot, relPath), 'utf8');
}

test('clears route state for signing payloads', async () => {
  const file = await read('pages/Signing.tsx');
  expect(file).toContain('replace: true');
  expect(file).toContain('state: null');
});

test('clears route state for send proposals', async () => {
  const file = await read('pages/SendConfirm.tsx');
  expect(file).toContain('replace: true');
  expect(file).toContain('state: null');
});

test('clears route state for swap flows', async () => {
  const quote = await read('pages/SwapQuote.tsx');
  const deposit = await read('pages/SwapDeposit.tsx');
  expect(quote).toContain('replace: true');
  expect(quote).toContain('state: null');
  expect(deposit).toContain('replace: true');
  expect(deposit).toContain('state: null');
});

test('clears seed phrase state on unmount', async () => {
  const file = await read('pages/SeedDisplay.tsx');
  expect(file).toContain('setWords([]');
  expect(file).toContain('onCleared()');
});

test('clears backup challenge inputs on unmount', async () => {
  const file = await read('pages/BackupChallenge.tsx');
  expect(file).toContain('setChallenge(null)');
  expect(file).toContain('setInputs({})');
});

test('clears view-seed dialog state when closed', async () => {
  const file = await read('components/common/ViewSeedPhraseDialog.tsx');
  expect(file).toContain('if (!open)');
  expect(file).toContain("setPassword('')");
  expect(file).toContain('setSeedWords(null)');
});

