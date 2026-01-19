import { expect, test } from 'bun:test';
import { formatAtomicAmount } from '../utils/amounts';

test('formatAtomicAmount formats various decimal places', () => {
  expect(formatAtomicAmount('1000000', 6)).toBe('1'); // USDC (6 decimals)
  expect(formatAtomicAmount('100000000', 8)).toBe('1'); // ZEC (8 decimals)
  expect(formatAtomicAmount('1000000000000000000', 18)).toBe('1'); // ETH (18 decimals)
});

test('formatAtomicAmount formats small values', () => {
  expect(formatAtomicAmount('0', 8)).toBe('0');
  expect(formatAtomicAmount('1', 18)).toBe('0.000000000000000001');
});

test('formatAtomicAmount passes through invalid input and decimals <= 0', () => {
  expect(formatAtomicAmount('  abc  ', 8)).toBe('abc');
  expect(formatAtomicAmount('123', 0)).toBe('123');
  expect(formatAtomicAmount('123', -1)).toBe('123');
});
