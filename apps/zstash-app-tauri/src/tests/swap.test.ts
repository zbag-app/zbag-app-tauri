import { expect, test } from 'bun:test';
import { parseSwapError } from '../utils/swap';

test('parseSwapError returns a user-friendly message for quote failures', () => {
  expect(parseSwapError('Failed to get quote: status=400')).toBe(
    'Failed to get quote. The amount may be below the minimum required or the swap pair is unavailable.'
  );
  expect(parseSwapError('Failed to get quote')).toBe(
    'Failed to get quote. The amount may be below the minimum required or the swap pair is unavailable.'
  );
});

test('parseSwapError passes through unrelated errors', () => {
  expect(parseSwapError('Something else happened')).toBe('Something else happened');
});
