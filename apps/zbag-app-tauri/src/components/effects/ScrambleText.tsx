import { useEffect, useState, useRef } from 'react';
import { useReducedMotion } from '../../hooks/useReducedMotion';

interface ScrambleTextProps {
  text: string;
  className?: string;
  delayMs?: number;
  duration?: number;
}

const GLYPHS = '!@#$%^&*()_+-=<>?/\\[]{}Xx';

/**
 * Runs the scramble animation using requestAnimationFrame.
 * Returns a cleanup function to cancel the animation.
 */
function runScrambleAnimation(
  text: string,
  duration: number,
  setDisplayText: (text: string) => void,
  onComplete?: () => void
): () => void {
  const startTime = performance.now();
  const totalChars = text.length;
  const finalChars = text.split('');
  const lockedIndices = new Set<number>();
  let frameId: number;
  let cancelled = false;

  function animate(currentTime: number) {
    if (cancelled) return;

    const elapsed = currentTime - startTime;
    const progress = Math.min(elapsed / (duration * 1000), 1);

    // Ease out
    const easedProgress = 1 - Math.pow(1 - progress, 2);
    const numLocked = Math.floor(easedProgress * totalChars);

    for (let i = 0; i < numLocked; i++) {
      lockedIndices.add(i);
    }

    const newDisplay = finalChars
      .map((char, i) => {
        if (lockedIndices.has(i)) return char;
        return GLYPHS[Math.floor(Math.random() * GLYPHS.length)];
      })
      .join('');

    setDisplayText(newDisplay);

    if (progress < 1) {
      frameId = requestAnimationFrame(animate);
    } else {
      setDisplayText(text);
      onComplete?.();
    }
  }

  frameId = requestAnimationFrame(animate);

  return () => {
    cancelled = true;
    cancelAnimationFrame(frameId);
  };
}

/**
 * Generate a scrambled version of the text using random glyphs.
 */
function scrambleText(text: string): string {
  return text
    .split('')
    .map(() => GLYPHS[Math.floor(Math.random() * GLYPHS.length)])
    .join('');
}

/**
 * ScrambleText - animates text on mount with a scramble effect.
 * Intentionally only animates once on first mount.
 */
export function ScrambleText({ text, className, delayMs = 0, duration = 0.9 }: ScrambleTextProps) {
  const reducedMotion = useReducedMotion();

  // Initialize with scrambled text if animation will run, final text otherwise
  const [displayText, setDisplayText] = useState(() => {
    if (reducedMotion) return text;
    return scrambleText(text);
  });

  const [hasAnimated, setHasAnimated] = useState(false);
  const timeoutRef = useRef<number | null>(null);
  const cancelAnimationRef = useRef<(() => void) | null>(null);
  const prevTextRef = useRef(text);

  // Animate on mount only (intentionally empty deps - animation runs once)
  useEffect(() => {
    if (hasAnimated || !text) return;

    // Skip animation if user prefers reduced motion
    if (reducedMotion) {
      setDisplayText(text);
      setHasAnimated(true);
      return;
    }

    timeoutRef.current = window.setTimeout(() => {
      cancelAnimationRef.current = runScrambleAnimation(text, duration, setDisplayText, () => {
        setHasAnimated(true);
      });
    }, delayMs);

    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
      if (cancelAnimationRef.current) cancelAnimationRef.current();
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps -- Intentionally animate only on first mount

  // Sync display text if text prop changes after animation completes
  useEffect(() => {
    if (hasAnimated && text !== prevTextRef.current) {
      setDisplayText(text);
    }
    prevTextRef.current = text;
  }, [text, hasAnimated]);

  return <span className={className}>{displayText || text}</span>;
}
