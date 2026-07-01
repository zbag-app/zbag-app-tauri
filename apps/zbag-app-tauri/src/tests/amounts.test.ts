import { expect, test } from 'bun:test';
import { formatAtomicAmount, formatAtomicAmountForToken } from '../utils/amounts';

test('formatAtomicAmount formats various decimal places', () => {
  expect(formatAtomicAmount('1000000', 6)).toBe('1'); // USDC (6 decimals)
  expect(formatAtomicAmount('100000000', 8)).toBe('1'); // ZEC (8 decimals)
  expect(formatAtomicAmount('1000000000000000000', 18)).toBe('1'); // ETH (18 decimals)
  expect(formatAtomicAmount('12345678', 8)).toBe('0.12345678'); // value length equals decimals
});

test('formatAtomicAmount handles 24 decimals (NEAR)', () => {
  expect(formatAtomicAmount('1000000000000000000000000', 24)).toBe('1');
});

test('formatAtomicAmount formats small values', () => {
  expect(formatAtomicAmount('0', 8)).toBe('0');
  expect(formatAtomicAmount('1', 18)).toBe('0.000000000000000001');
});

test('formatAtomicAmount handles edge cases', () => {
  expect(formatAtomicAmount('', 8)).toBe('');
  expect(formatAtomicAmount('00123', 8)).toBe('0.00000123');
  expect(formatAtomicAmount('0000000001', 8)).toBe('0.00000001');
  expect(formatAtomicAmount('1000000000000000000000000', 18)).toBe('1000000');
});

test('formatAtomicAmount passes through invalid input and decimals <= 0', () => {
  expect(formatAtomicAmount('  abc  ', 8)).toBe('abc');
  expect(formatAtomicAmount('123', 0)).toBe('123');
  expect(formatAtomicAmount('123', -1)).toBe('123');
});

test('formatAtomicAmount passes through extremely large numeric strings', () => {
  const huge = '1'.repeat(79);
  expect(formatAtomicAmount(huge, 8)).toBe(huge);
});

test('formatAtomicAmountForToken formats known tokens and falls back for unknown tokens', () => {
  expect(formatAtomicAmountForToken('100000000', 'nep141:zec.omft.near')).toEqual({
    value: '1 ZEC',
    isRaw: false,
  });
  expect(formatAtomicAmountForToken('  123  ', 'unknown-token')).toEqual({ value: '123', isRaw: true });
  expect(formatAtomicAmountForToken('abc', 'nep141:zec.omft.near')).toEqual({ value: 'abc', isRaw: true });
  expect(formatAtomicAmountForToken('   ', 'unknown-token')).toEqual({ value: '', isRaw: false });
});
