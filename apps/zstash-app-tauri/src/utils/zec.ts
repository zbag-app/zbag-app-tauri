import { formatAtomicAmount } from './amounts';

const ZATOSHI_PER_ZEC = 100_000_000n;

export type ParseAmountResult = { ok: string } | { err: string };

export function parseZecToZatoshis(input: string): ParseAmountResult {
  const value = input.trim();
  if (!value) return { err: 'Enter an amount.' };
  if (value.startsWith('-')) return { err: 'Amount must be positive.' };
  if (!/^\d*(?:\.\d*)?$/.test(value)) return { err: 'Use only digits and a single decimal point.' };

  const [wholeRaw, fractionalRaw = ''] = value.split('.', 2);
  const whole = wholeRaw.length ? wholeRaw : '0';

  if (fractionalRaw.length > 8) {
    const extra = fractionalRaw.slice(8);
    if (!/^0*$/.test(extra)) return { err: 'ZEC supports up to 8 decimal places.' };
  }

  const fractional = fractionalRaw.slice(0, 8).padEnd(8, '0');

  const zatoshis = BigInt(whole) * ZATOSHI_PER_ZEC + BigInt(fractional);
  if (zatoshis <= 0n) return { err: 'Amount must be greater than 0.' };

  return { ok: zatoshis.toString(10) };
}

export function formatZatoshisToZec(input: string): string {
  return formatAtomicAmount(input, 8);
}

export function formatRelativeTime(timestampMs: number): string {
  const now = Date.now();
  const diffMs = now - timestampMs;
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffSec < 60) return 'just now';
  if (diffMin < 60) return `${diffMin} minute${diffMin === 1 ? '' : 's'} ago`;
  if (diffHour < 24) return `${diffHour} hour${diffHour === 1 ? '' : 's'} ago`;
  if (diffDay < 30) return `${diffDay} day${diffDay === 1 ? '' : 's'} ago`;

  const diffMonth = Math.floor(diffDay / 30);
  if (diffMonth < 12) return `${diffMonth} month${diffMonth === 1 ? '' : 's'} ago`;

  const diffYear = Math.floor(diffMonth / 12);
  return `${diffYear} year${diffYear === 1 ? '' : 's'} ago`;
}

