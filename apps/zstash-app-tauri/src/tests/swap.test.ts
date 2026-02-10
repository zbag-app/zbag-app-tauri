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
  expect(parseSwapError('Failed to start swap: status=400')).toBe('Failed to start swap: status=400');
});

test('parseSwapError extracts upstream JSON body messages when present', () => {
  expect(
    parseSwapError(
      'failed to fetch quote: http error: status=400 {"message":"refundTo is not valid","correlationId":"x"}'
    )
  ).toBe('Refund address is invalid. Use a transparent (t-) Zcash address for swap refunds.');
});
