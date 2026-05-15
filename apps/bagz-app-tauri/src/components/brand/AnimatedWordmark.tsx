import { useEffect, useState } from 'react';
import { ScrambleText } from '../effects/ScrambleText';
import { useReducedMotion } from '../../hooks/useReducedMotion';

interface AnimatedWordmarkProps {
  showTagline?: boolean;
  className?: string;
}

const BRAND_NAME = 'bagZ';
const LETTER_OFFSET_PX = 8;

/**
 * Animated bagZ wordmark with optional tagline.
 * Letters animate in with a staggered reveal effect.
 * Respects prefers-reduced-motion accessibility setting.
 */
export function AnimatedWordmark({ showTagline = false, className = '' }: AnimatedWordmarkProps) {
  const reducedMotion = useReducedMotion();
  const [visible, setVisible] = useState(reducedMotion);

  useEffect(() => {
    if (reducedMotion) return;
    // Small delay before starting animation
    const timer = setTimeout(() => setVisible(true), 100);
    return () => clearTimeout(timer);
  }, [reducedMotion]);

  const letters = BRAND_NAME.split('');

  return (
    <div className={`flex flex-col items-center ${className}`}>
      <div className="flex items-baseline" aria-label="bagZ">
        {letters.map((letter, index) => (
          <span
            key={`letter-${index}-${letter}`}
            aria-hidden="true"
            className="font-display text-7xl font-bold tracking-tight transition-all duration-300"
            style={{
              opacity: visible ? 1 : 0,
              transform: visible ? 'translateY(0)' : `translateY(${LETTER_OFFSET_PX}px)`,
              transitionDelay: reducedMotion ? '0ms' : `${index * 50}ms`,
            }}
          >
            {letter}
          </span>
        ))}
      </div>
      {showTagline && (
        <p
          className="font-mono text-3xl text-muted-foreground mt-3 transition-opacity duration-500"
          style={{
            opacity: visible ? 1 : 0,
            transitionDelay: reducedMotion ? '0ms' : '400ms',
          }}
        >
          <ScrambleText text="encrypt your wealth" delayMs={500} duration={0.8} />
        </p>
      )}
    </div>
  );
}
