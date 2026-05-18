import * as React from 'react';
import { ChevronDown, Check } from 'lucide-react';
import { cn } from '../../lib/utils';
import type { FiatCurrency } from '../../types/ipc';
import { FIAT_CURRENCIES, FIAT_CURRENCY_SYMBOLS } from '../../types/ipc';

const CURRENCY_NAMES: Record<FiatCurrency, string> = {
  USD: 'US Dollar',
  EUR: 'Euro',
  GBP: 'British Pound',
  CHF: 'Swiss Franc',
  CAD: 'Canadian Dollar',
  AUD: 'Australian Dollar',
  JPY: 'Japanese Yen',
};

interface FiatCurrencySelectProps {
  value: FiatCurrency;
  onChange: (currency: FiatCurrency) => void;
  disabled?: boolean;
}

export function FiatCurrencySelect({ value, onChange, disabled }: FiatCurrencySelectProps) {
  const [open, setOpen] = React.useState(false);
  const [highlightedIndex, setHighlightedIndex] = React.useState(-1);
  const containerRef = React.useRef<HTMLDivElement>(null);
  const listRef = React.useRef<HTMLUListElement>(null);

  const selectedIndex = FIAT_CURRENCIES.indexOf(value);

  // Close on outside click
  React.useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Keyboard navigation
  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (disabled) return;

    switch (event.key) {
      case 'Enter':
      case ' ':
        event.preventDefault();
        if (open && highlightedIndex >= 0) {
          onChange(FIAT_CURRENCIES[highlightedIndex]);
          setOpen(false);
        } else {
          setOpen(!open);
          setHighlightedIndex(selectedIndex);
        }
        break;
      case 'Escape':
        setOpen(false);
        break;
      case 'ArrowDown':
        event.preventDefault();
        if (!open) {
          setOpen(true);
          setHighlightedIndex(selectedIndex);
        } else {
          setHighlightedIndex((prev) =>
            prev < FIAT_CURRENCIES.length - 1 ? prev + 1 : 0
          );
        }
        break;
      case 'ArrowUp':
        event.preventDefault();
        if (!open) {
          setOpen(true);
          setHighlightedIndex(selectedIndex);
        } else {
          setHighlightedIndex((prev) =>
            prev > 0 ? prev - 1 : FIAT_CURRENCIES.length - 1
          );
        }
        break;
      case 'Tab':
        setOpen(false);
        break;
    }
  };

  // Scroll highlighted item into view
  React.useEffect(() => {
    if (open && highlightedIndex >= 0 && listRef.current) {
      const item = listRef.current.children[highlightedIndex] as HTMLElement;
      item?.scrollIntoView({ block: 'nearest' });
    }
  }, [highlightedIndex, open]);

  return (
    <div ref={containerRef} className="relative w-full">
      {/* Trigger Button */}
      <button
        type="button"
        onClick={() => !disabled && setOpen(!open)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-labelledby="fiat-currency-label"
        className={cn(
          'w-full flex items-center justify-between gap-3 px-4 py-3',
          'bg-input border border-border',
          'text-foreground',
          'transition-all duration-200',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          !disabled && 'hover:border-primary/50 glow-gold'
        )}
      >
        <div className="flex items-center gap-3">
          <span className="font-mono text-xl text-primary font-semibold w-8">
            {FIAT_CURRENCY_SYMBOLS[value]}
          </span>
          <span className="text-sm">
            {CURRENCY_NAMES[value]}
          </span>
        </div>
        <ChevronDown
          className={cn(
            'h-4 w-4 text-muted-foreground transition-transform duration-200',
            open && 'rotate-180'
          )}
        />
      </button>

      {/* Dropdown Panel */}
      {open && (
        <ul
          ref={listRef}
          role="listbox"
          aria-activedescendant={highlightedIndex >= 0 ? `currency-${FIAT_CURRENCIES[highlightedIndex]}` : undefined}
          className={cn(
            'absolute z-50 w-full mt-1',
            'bg-card border border-border',
            'max-h-64 overflow-auto',
            'animate-[scale-in_0.15s_ease-out]',
            'shadow-lg shadow-black/20'
          )}
        >
          {FIAT_CURRENCIES.map((currency, index) => {
            const isSelected = currency === value;
            const isHighlighted = index === highlightedIndex;

            return (
              <li
                key={currency}
                id={`currency-${currency}`}
                role="option"
                aria-selected={isSelected}
                onClick={() => {
                  onChange(currency);
                  setOpen(false);
                }}
                onMouseEnter={() => setHighlightedIndex(index)}
                className={cn(
                  'flex items-center gap-3 px-4 py-3 cursor-pointer',
                  'transition-colors duration-100',
                  isHighlighted && 'bg-muted',
                  isSelected && 'border-l-2 border-l-primary bg-primary/5'
                )}
              >
                <span
                  className={cn(
                    'font-mono text-xl font-semibold w-8',
                    isSelected ? 'text-primary' : 'text-muted-foreground'
                  )}
                >
                  {FIAT_CURRENCY_SYMBOLS[currency]}
                </span>
                <span className="flex-1 text-sm">
                  {CURRENCY_NAMES[currency]}
                </span>
                <span className="text-xs text-muted-foreground font-mono">
                  {currency}
                </span>
                {isSelected && (
                  <Check className="h-4 w-4 text-primary" />
                )}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
