import { getTokenById } from '../data/supportedTokens';

export type FormattedAmountForToken = {
  value: string;
  isRaw: boolean;
};

/**
 * Format an atomic/smallest-unit amount to a human-readable decimal string.
 *
 * @param value - The amount in smallest units (e.g., zatoshis, wei) as a string
 * @param decimals - Number of decimal places (e.g., 8 for ZEC, 18 for ETH)
 * @note `value` must be a base-10 non-negative integer string. Whitespace is trimmed; other formats (including empty)
 * are returned unchanged after trim.
 * @returns Formatted decimal string (e.g., "1.5" for 150000000 zatoshis)
 */
export function formatAtomicAmount(value: string, decimals: number): string {
  const trimmed = value.trim();
  if (!/^\d+$/.test(trimmed)) return trimmed;
  // Prevent pathological inputs from causing large allocations and slow formatting in the UI.
  if (trimmed.length > 78) return trimmed;
  if (decimals <= 0) return trimmed;

  const padded = trimmed.padStart(decimals + 1, '0');
  const wholeEnd = padded.length - decimals;
  const wholeRaw = padded.slice(0, wholeEnd);
  const whole = wholeRaw.replace(/^0+/, '') || '0';
  const fractional = padded.slice(wholeEnd).replace(/0+$/, '');

  if (!fractional) return whole;
  return `${whole}.${fractional}`;
}

/**
 * Format an atomic amount using known token metadata.
 *
 * Returns `{ value: '', isRaw: false }` for empty/whitespace input.
 *
 * Falls back to showing the raw atomic value with `isRaw=true` if the token is unknown or the input is not a base-10
 * integer string.
 */
export function formatAtomicAmountForToken(value: string, tokenId: string): FormattedAmountForToken {
  const trimmed = value.trim();
  if (!trimmed) return { value: '', isRaw: false };

  const token = getTokenById(tokenId);
  if (!token) return { value: trimmed, isRaw: true };
  if (!/^\d+$/.test(trimmed)) return { value: trimmed, isRaw: true };
  return { value: `${formatAtomicAmount(trimmed, token.decimals)} ${token.label}`, isRaw: false };
}
