import { useCallback, useRef, useEffect } from 'react';

export function useThrottledCallback<T extends (...args: any[]) => void>(
  callback: T,
  delay: number
): T {
  // Store callback in ref to avoid identity changes causing resubscription
  const callbackRef = useRef(callback);
  const lastCall = useRef(0);
  const lastArgs = useRef<Parameters<T> | null>(null);
  const timeoutId = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Keep callback ref updated without changing throttled function identity
  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  // Cleanup pending timeout on unmount
  useEffect(() => {
    return () => {
      if (timeoutId.current) {
        clearTimeout(timeoutId.current);
      }
    };
  }, []);

  // Return stable function that only depends on delay
  return useCallback(
    ((...args: Parameters<T>) => {
      const now = Date.now();
      lastArgs.current = args;

      if (now - lastCall.current >= delay) {
        lastCall.current = now;
        callbackRef.current(...args);
      } else if (!timeoutId.current) {
        const remaining = delay - (now - lastCall.current);
        timeoutId.current = setTimeout(() => {
          lastCall.current = Date.now();
          if (lastArgs.current) {
            callbackRef.current(...lastArgs.current);
          }
          timeoutId.current = null;
        }, remaining);
      }
    }) as T,
    [delay] // Only depends on delay, NOT callback (stable identity)
  );
}
