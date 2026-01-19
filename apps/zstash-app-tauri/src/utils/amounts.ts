import { getTokenById } from '../data/supportedTokens';

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
 * Falls back to showing the raw atomic value if the token is unknown.
 */
export function formatAtomicAmountForToken(value: string, tokenId: string): string {
  const trimmed = value.trim();
  if (!trimmed) return '';

  const token = getTokenById(tokenId);
  if (!token) return `${trimmed} (raw)`;
  if (!/^\d+$/.test(trimmed)) return `${trimmed} (raw)`;
  return `${formatAtomicAmount(trimmed, token.decimals)} ${token.label}`;
}
