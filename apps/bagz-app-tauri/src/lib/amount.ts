type DecimalAmountValidationOptions = {
  maxDecimals: number;
  example?: string;
};

export function validateDecimalAmount(value: string, opts: DecimalAmountValidationOptions): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  if (!/^[0-9]+(?:\.[0-9]*)?$/.test(trimmed)) {
    const example = opts.example ?? '1.23';
    return `Enter a valid amount (e.g., ${example})`;
  }

  const [, frac = ''] = trimmed.split('.');
  if (frac.length > opts.maxDecimals) return `Too many decimal places (max ${opts.maxDecimals})`;

  if (/^0+(?:\.0*)?$/.test(trimmed)) return 'Amount must be greater than zero';
  return null;
}

