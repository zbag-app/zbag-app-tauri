import { cn } from '../../../lib/utils';
import { getChainInfo } from '../../../data/chainMetadata';

interface ChainBadgeProps {
  chain: string;
  className?: string;
}

export function ChainBadge({ chain, className }: ChainBadgeProps) {
  const chainInfo = getChainInfo(chain);

  return (
    <span
      className={cn(
        'inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide',
        chainInfo.color,
        className
      )}
    >
      {chainInfo.name}
    </span>
  );
}
