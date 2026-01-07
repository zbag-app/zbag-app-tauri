import { expect, test } from 'bun:test';
import { formatZatoshisToZec, parseZecToZatoshis } from '../utils/zec';

test('parseZecToZatoshis parses whole ZEC', () => {
  expect(parseZecToZatoshis('1')).toEqual({ ok: '100000000' });
});

test('parseZecToZatoshis parses fractional ZEC', () => {
  expect(parseZecToZatoshis('0.1')).toEqual({ ok: '10000000' });
  expect(parseZecToZatoshis('.1')).toEqual({ ok: '10000000' });
  expect(parseZecToZatoshis('1.23456789')).toEqual({ ok: '123456789' });
});

test('parseZecToZatoshis rejects more than 8 decimals (unless trailing zeros)', () => {
  expect(parseZecToZatoshis('1.234567890')).toEqual({ ok: '123456789' });
  expect(parseZecToZatoshis('1.234567891')).toEqual({ err: 'ZEC supports up to 8 decimal places.' });
});

test('parseZecToZatoshis rejects empty and zero', () => {
  expect(parseZecToZatoshis('')).toEqual({ err: 'Enter an amount.' });
  expect(parseZecToZatoshis('0')).toEqual({ err: 'Amount must be greater than 0.' });
});

test('formatZatoshisToZec formats without trailing zeros', () => {
  expect(formatZatoshisToZec('100000000')).toBe('1');
  expect(formatZatoshisToZec('123456789')).toBe('1.23456789');
  expect(formatZatoshisToZec('100000001')).toBe('1.00000001');
});

test('formatZatoshisToZec passes through invalid input', () => {
  expect(formatZatoshisToZec('abc')).toBe('abc');
});

