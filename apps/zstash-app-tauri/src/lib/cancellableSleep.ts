export function createCancellableSleep() {
  let timeoutId: number | null = null;
  let resolveSleep: (() => void) | null = null;

  const sleep = (ms: number) =>
    new Promise<void>((resolve) => {
      resolveSleep = resolve;
      timeoutId = window.setTimeout(() => {
        timeoutId = null;
        resolveSleep = null;
        resolve();
      }, ms);
    });

  const cancel = () => {
    if (timeoutId != null) {
      window.clearTimeout(timeoutId);
      timeoutId = null;
    }
    resolveSleep?.();
    resolveSleep = null;
  };

  return { sleep, cancel };
}
