import { useEffect, type RefObject } from 'react';

export function useFocusTrap(containerRef: RefObject<HTMLElement>, enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    const container = containerRef.current;
    if (!container) return;

    const focusable = container.querySelector<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    focusable?.focus();
  }, [containerRef, enabled]);
}

