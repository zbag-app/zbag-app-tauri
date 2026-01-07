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
  const value = input.trim();
  if (!/^\d+$/.test(value)) return input;

  const zatoshis = BigInt(value);
  const whole = zatoshis / ZATOSHI_PER_ZEC;
  const fractional = zatoshis % ZATOSHI_PER_ZEC;
  if (fractional === 0n) return whole.toString(10);

  const fractionalStr = fractional
    .toString(10)
    .padStart(8, '0')
    .replace(/0+$/, '');
  return `${whole.toString(10)}.${fractionalStr}`;
}

