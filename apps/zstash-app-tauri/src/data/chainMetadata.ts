export interface ChainInfo {
  name: string;
  color: string;
}

export const CHAIN_METADATA: Record<string, ChainInfo> = {
  zec: { name: 'Zcash', color: 'text-primary bg-primary/10' },
  eth: { name: 'Ethereum', color: 'text-blue-400 bg-blue-400/10' },
  sol: { name: 'Solana', color: 'text-purple-400 bg-purple-400/10' },
  near: { name: 'NEAR', color: 'text-green-400 bg-green-400/10' },
  base: { name: 'Base', color: 'text-blue-300 bg-blue-300/10' },
  btc: { name: 'Bitcoin', color: 'text-orange-400 bg-orange-400/10' },
  arb: { name: 'Arbitrum', color: 'text-blue-500 bg-blue-500/10' },
  aurora: { name: 'Aurora', color: 'text-emerald-400 bg-emerald-400/10' },
  turbochain: { name: 'TurboChain', color: 'text-cyan-400 bg-cyan-400/10' },
};

export function getChainInfo(chain: string): ChainInfo {
  return CHAIN_METADATA[chain] ?? { name: chain.toUpperCase(), color: 'text-muted-foreground bg-muted/50' };
}
