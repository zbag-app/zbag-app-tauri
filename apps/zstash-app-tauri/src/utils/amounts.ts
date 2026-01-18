/**
 * Format an atomic/smallest-unit amount to a human-readable decimal string.
 *
 * @param value - The amount in smallest units (e.g., zatoshis, wei) as a string
 * @param decimals - Number of decimal places (e.g., 8 for ZEC, 18 for ETH)
 * @returns Formatted decimal string (e.g., "1.5" for 150000000 zatoshis)
 */
export function formatAtomicAmount(value: string, decimals: number): string {
  const trimmed = value.trim();
  if (!/^\d+$/.test(trimmed)) return trimmed;
  if (decimals <= 0) return trimmed;

  const padded = trimmed.padStart(decimals + 1, '0');
  const wholeEnd = padded.length - decimals;
  const whole = padded.slice(0, wholeEnd);
  const fractional = padded.slice(wholeEnd).replace(/0+$/, '');

  if (!fractional) return whole;
  return `${whole}.${fractional}`;
}
