export function createCancellableSleep() {
  let timeoutId: number | null = null;
  let resolveSleep: (() => void) | null = null;
  const clearPending = () => {
    if (timeoutId != null) {
      window.clearTimeout(timeoutId);
      timeoutId = null;
    }
    // Cancellation resolves the pending sleep promise (it does not reject).
    resolveSleep?.();
    resolveSleep = null;
  };

  const sleep = (ms: number) =>
    new Promise<void>((resolve) => {
      // Ensure there is at most one outstanding sleep promise.
      clearPending();
      resolveSleep = resolve;
      timeoutId = window.setTimeout(() => {
        timeoutId = null;
        resolveSleep = null;
        resolve();
      }, ms);
    });

  const cancel = () => {
    clearPending();
  };

  return { sleep, cancel };
}
