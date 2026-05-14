import * as React from 'react';
import { ChevronDown, Check, Loader2, Search } from 'lucide-react';
import { cn } from '../../../lib/utils';
import type { SupportedToken } from '../../../data/supportedTokens';
import { getChainInfo } from '../../../data/chainMetadata';
import { TokenIcon } from './TokenIcon';
import { ChainBadge } from './ChainBadge';

interface TokenPickerProps {
  value: string;
  onChange: (tokenId: string) => void;
  tokens: SupportedToken[];
  disabled?: boolean;
  loading?: boolean;
  placeholder?: string;
  id?: string;
}

interface GroupedTokens {
  chain: string;
  chainName: string;
  tokens: SupportedToken[];
}

function groupTokensByChain(tokens: SupportedToken[]): GroupedTokens[] {
  const groups = new Map<string, SupportedToken[]>();

  for (const token of tokens) {
    const existing = groups.get(token.chain);
    if (existing) {
      existing.push(token);
    } else {
      groups.set(token.chain, [token]);
    }
  }

  return Array.from(groups.entries()).map(([chain, chainTokens]) => ({
    chain,
    chainName: getChainInfo(chain).name,
    tokens: chainTokens,
  }));
}

function formatUsdPrice(price: number | null): string | null {
  if (price == null) return null;
  if (price >= 1) {
    return `$${price.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
  }
  return `$${price.toFixed(4)}`;
}

export function TokenPicker({
  value,
  onChange,
  tokens,
  disabled = false,
  loading = false,
  placeholder = 'Select token',
  id,
}: TokenPickerProps) {
  const [open, setOpen] = React.useState(false);
  const [searchQuery, setSearchQuery] = React.useState('');
  const [highlightedIndex, setHighlightedIndex] = React.useState(-1);

  const containerRef = React.useRef<HTMLDivElement>(null);
  const listRef = React.useRef<HTMLDivElement>(null);
  const searchInputRef = React.useRef<HTMLInputElement>(null);

  const selectedToken = React.useMemo(
    () => tokens.find((t) => t.asset_id === value),
    [tokens, value]
  );

  // Filter tokens based on search query
  const filteredTokens = React.useMemo(() => {
    if (!searchQuery.trim()) return tokens;
    const query = searchQuery.toLowerCase();
    return tokens.filter(
      (t) =>
        t.symbol.toLowerCase().includes(query) ||
        t.chain.toLowerCase().includes(query) ||
        getChainInfo(t.chain).name.toLowerCase().includes(query)
    );
  }, [tokens, searchQuery]);

  // Group filtered tokens by chain
  const groupedTokens = React.useMemo(
    () => groupTokensByChain(filteredTokens),
    [filteredTokens]
  );

  // Flatten for keyboard navigation
  const flatTokens = React.useMemo(
    () => groupedTokens.flatMap((g) => g.tokens),
    [groupedTokens]
  );

  const selectedIndex = flatTokens.findIndex((t) => t.asset_id === value);

  // Close on outside click
  React.useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false);
        setSearchQuery('');
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  // Focus search input when opened
  React.useEffect(() => {
    if (open && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [open]);

  // Reset highlight when filtered tokens change
  React.useEffect(() => {
    if (open && flatTokens.length > 0) {
      const currentSelected = flatTokens.findIndex((t) => t.asset_id === value);
      setHighlightedIndex(currentSelected >= 0 ? currentSelected : 0);
    }
  }, [open, flatTokens, value]);

  // Scroll highlighted item into view
  React.useEffect(() => {
    if (open && highlightedIndex >= 0 && listRef.current) {
      const item = listRef.current.querySelector(`[data-index="${highlightedIndex}"]`) as HTMLElement;
      item?.scrollIntoView({ block: 'nearest' });
    }
  }, [highlightedIndex, open]);

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (disabled || loading) return;

    switch (event.key) {
      case ' ':
        // Allow space to be typed in the search input
        if (event.target === searchInputRef.current) {
          return;
        }
        event.preventDefault();
        if (open && highlightedIndex >= 0 && flatTokens[highlightedIndex]) {
          onChange(flatTokens[highlightedIndex].asset_id);
          setOpen(false);
          setSearchQuery('');
        } else if (!open) {
          setOpen(true);
          setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
        }
        break;
      case 'Enter':
        event.preventDefault();
        if (open && highlightedIndex >= 0 && flatTokens[highlightedIndex]) {
          onChange(flatTokens[highlightedIndex].asset_id);
          setOpen(false);
          setSearchQuery('');
        } else if (!open) {
          setOpen(true);
          setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
        }
        break;
      case 'Escape':
        setOpen(false);
        setSearchQuery('');
        break;
      case 'ArrowDown':
        event.preventDefault();
        if (!open) {
          setOpen(true);
          setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
        } else {
          setHighlightedIndex((prev) =>
            prev < flatTokens.length - 1 ? prev + 1 : 0
          );
        }
        break;
      case 'ArrowUp':
        event.preventDefault();
        if (!open) {
          setOpen(true);
          setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
        } else {
          setHighlightedIndex((prev) =>
            prev > 0 ? prev - 1 : flatTokens.length - 1
          );
        }
        break;
      case 'Tab':
        setOpen(false);
        setSearchQuery('');
        break;
    }
  };

  const handleSearchKeyDown = (event: React.KeyboardEvent) => {
    // Let arrow keys and enter propagate to container handler
    if (['ArrowDown', 'ArrowUp', 'Enter', 'Escape', 'Tab'].includes(event.key)) {
      handleKeyDown(event);
    }
  };

  if (loading) {
    return (
      <div className="flex h-9 items-center gap-2 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        Loading tokens...
      </div>
    );
  }

  return (
    <div ref={containerRef} className="relative w-full">
      {/* Trigger Button */}
      <button
        type="button"
        id={id}
        onClick={() => !disabled && setOpen(!open)}
        onKeyDown={handleKeyDown}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-labelledby={id ? `${id}-label` : undefined}
        className={cn(
          'w-full flex items-center justify-between gap-2 h-9 px-3',
          'bg-input border border-border',
          'text-foreground text-sm',
          'transition-all duration-200',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          !disabled && 'hover:border-primary/50 glow-gold'
        )}
      >
        {selectedToken ? (
          <div className="flex items-center gap-2 min-w-0 flex-1">
            <TokenIcon
              symbol={selectedToken.symbol}
              chain={selectedToken.chain}
              iconUrl={selectedToken.icon}
              size="sm"
            />
            <span className="font-medium truncate">{selectedToken.symbol}</span>
            <ChainBadge chain={selectedToken.chain} />
            {selectedToken.usd_price != null && (
              <span className="text-xs text-muted-foreground ml-auto">
                {formatUsdPrice(selectedToken.usd_price)}
              </span>
            )}
          </div>
        ) : (
          <span className="text-muted-foreground">{placeholder}</span>
        )}
        <ChevronDown
          className={cn(
            'h-4 w-4 text-muted-foreground shrink-0 transition-transform duration-200',
            open && 'rotate-180'
          )}
        />
      </button>

      {/* Dropdown Panel */}
      {open && (
        <div
          className={cn(
            'absolute z-50 w-full mt-1',
            'bg-card border border-border',
            'animate-[scale-in_0.15s_ease-out]',
            'shadow-lg shadow-black/20'
          )}
        >
          {/* Search Input */}
          <div className="border-b border-border p-2">
            <div className="relative">
              <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <input
                ref={searchInputRef}
                type="text"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                onKeyDown={handleSearchKeyDown}
                placeholder="Search by symbol or chain..."
                className={cn(
                  'w-full h-8 pl-8 pr-3',
                  'bg-input border border-border',
                  'text-sm text-foreground placeholder:text-muted-foreground',
                  'focus:outline-none focus:ring-1 focus:ring-ring'
                )}
              />
            </div>
          </div>

          {/* Token List */}
          <div
            ref={listRef}
            role="listbox"
            aria-activedescendant={
              highlightedIndex >= 0 && flatTokens[highlightedIndex]
                ? `token-${flatTokens[highlightedIndex].asset_id}`
                : undefined
            }
            className="max-h-64 overflow-auto"
          >
            {groupedTokens.length === 0 ? (
              <div className="px-3 py-4 text-sm text-muted-foreground text-center">
                No tokens found
              </div>
            ) : (
              groupedTokens.map((group) => {
                // Calculate starting index for this group
                let groupStartIndex = 0;
                for (const g of groupedTokens) {
                  if (g.chain === group.chain) break;
                  groupStartIndex += g.tokens.length;
                }

                return (
                  <div key={group.chain}>
                    {/* Chain Header */}
                    <div className="sticky top-0 px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground bg-muted/30">
                      {group.chainName}
                    </div>
                    {/* Tokens */}
                    {group.tokens.map((token, i) => {
                      const flatIndex = groupStartIndex + i;
                      const isSelected = token.asset_id === value;
                      const isHighlighted = flatIndex === highlightedIndex;

                      return (
                        <div
                          key={token.asset_id}
                          id={`token-${token.asset_id}`}
                          role="option"
                          aria-selected={isSelected}
                          data-index={flatIndex}
                          onClick={() => {
                            onChange(token.asset_id);
                            setOpen(false);
                            setSearchQuery('');
                          }}
                          onMouseEnter={() => setHighlightedIndex(flatIndex)}
                          className={cn(
                            'flex items-center gap-2 px-3 py-2 cursor-pointer',
                            'transition-colors duration-100',
                            isHighlighted && 'bg-primary/20',
                            isSelected && 'border-l-2 border-l-primary bg-primary/5'
                          )}
                        >
                          <TokenIcon
                            symbol={token.symbol}
                            chain={token.chain}
                            iconUrl={token.icon}
                            size="md"
                          />
                          <span className={cn('font-medium', isSelected && 'text-primary')}>
                            {token.symbol}
                          </span>
                          <ChainBadge chain={token.chain} className="ml-1" />
                          {token.usd_price != null && (
                            <span className="text-xs text-muted-foreground ml-auto">
                              {formatUsdPrice(token.usd_price)}
                            </span>
                          )}
                          {isSelected && <Check className="h-4 w-4 text-primary shrink-0" />}
                        </div>
                      );
                    })}
                  </div>
                );
              })
            )}
          </div>
        </div>
      )}
    </div>
  );
}
