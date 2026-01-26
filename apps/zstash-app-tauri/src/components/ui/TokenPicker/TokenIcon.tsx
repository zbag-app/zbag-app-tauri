import * as React from 'react';
import { cn } from '../../../lib/utils';
import { getChainInfo } from '../../../data/chainMetadata';

interface TokenIconProps {
  symbol: string;
  chain: string;
  iconUrl: string | null;
  size?: 'sm' | 'md';
  className?: string;
}

export function TokenIcon({ symbol, chain, iconUrl, size = 'md', className }: TokenIconProps) {
  const [imgError, setImgError] = React.useState(false);
  const chainInfo = getChainInfo(chain);

  const sizeClasses = size === 'sm' ? 'h-5 w-5 text-xs' : 'h-6 w-6 text-sm';

  // Reset error state when iconUrl changes
  React.useEffect(() => {
    setImgError(false);
  }, [iconUrl]);

  if (iconUrl && !imgError) {
    return (
      <img
        src={iconUrl}
        alt={symbol}
        className={cn(sizeClasses, 'object-contain', className)}
        onError={() => setImgError(true)}
      />
    );
  }

  // Fallback: first letter of symbol in colored square
  return (
    <div
      className={cn(
        sizeClasses,
        'flex items-center justify-center font-semibold',
        chainInfo.color,
        className
      )}
    >
      {symbol.charAt(0).toUpperCase()}
    </div>
  );
}
