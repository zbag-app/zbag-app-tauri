/** Parse API error messages for user-friendly display */
export function parseSwapError(message: string): string {
  if (message.includes('Failed to get quote') || message.includes('status=400')) {
    return 'Failed to get quote. The amount may be below the minimum required or the swap pair is unavailable.';
  }
  return message;
}
