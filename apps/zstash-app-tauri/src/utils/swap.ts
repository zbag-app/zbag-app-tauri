/**
 * Parse known swap quote API error patterns for user-friendly display.
 *
 * The upstream 1Click API sometimes returns a 400 with "Failed to get quote", and the backend already keys off that
 * message for retries.
 */
export const PRIVACY_ACK_REQUIRED_MESSAGE =
  'This swap requires transparent interaction. Confirm the privacy acknowledgement to continue.';

export function parseSwapError(message: string): string {
  const trimmed = message.trim();
  const lower = trimmed.toLowerCase();

  if (lower.includes('failed to get quote')) {
    return 'Failed to get quote. The amount may be below the minimum required or the swap pair is unavailable.';
  }

  // Some backend errors include only status codes (e.g. "RequestSwapQuote failed: status=400").
  // Keep this match narrow to quote-related failures to avoid false positives.
  if (lower.includes('quote') && lower.includes('status=400')) {
    return 'Failed to get quote. The amount may be below the minimum required or the swap pair is unavailable.';
  }

  return trimmed;
}
