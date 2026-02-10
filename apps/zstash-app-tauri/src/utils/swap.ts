/**
 * Parse known swap quote API error patterns for user-friendly display.
 *
 * The upstream 1Click API sometimes returns a 400 with "Failed to get quote", and the backend already keys off that
 * message for retries.
 */
export const PRIVACY_ACK_REQUIRED_MESSAGE =
  'This swap requires transparent interaction. Confirm the privacy acknowledgement to continue.';

function tryExtractJsonMessage(input: string): string | null {
  // Backend errors often embed the upstream JSON body, e.g.:
  // "failed to fetch quote: http error: status=400 {\"message\":\"refundTo is not valid\",...}"
  const first = input.indexOf('{');
  const last = input.lastIndexOf('}');
  if (first === -1 || last === -1 || last <= first) return null;

  const candidate = input.slice(first, last + 1);
  try {
    const parsed: unknown = JSON.parse(candidate);
    if (
      parsed != null &&
      typeof parsed === 'object' &&
      'message' in parsed &&
      typeof (parsed as { message?: unknown }).message === 'string'
    ) {
      return (parsed as { message: string }).message.trim();
    }
  } catch {
    // Ignore parse errors; fall back to the original input.
  }
  return null;
}

export function parseSwapError(message: string): string {
  const trimmed = message.trim();
  const extracted = tryExtractJsonMessage(trimmed);
  const normalized = (extracted ?? trimmed).trim();
  const lower = normalized.toLowerCase();

  if (lower.includes('failed to get quote')) {
    return 'Failed to get quote. The amount may be below the minimum required or the swap pair is unavailable.';
  }

  // Make common upstream validation errors actionable.
  if (lower === 'refundto is not valid') {
    return 'Refund address is invalid. Use a transparent (t-) Zcash address for swap refunds.';
  }

  return normalized;
}
