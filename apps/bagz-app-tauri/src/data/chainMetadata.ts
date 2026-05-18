export interface ChainInfo {
  name: string;
  color: string;
}

export const CHAIN_METADATA: Record<string, ChainInfo> = {
  // Primary chains
  zec: { name: 'Zcash', color: 'text-primary bg-primary/10' },
  eth: { name: 'Ethereum', color: 'text-blue-400 bg-blue-400/10' },
  sol: { name: 'Solana', color: 'text-purple-400 bg-purple-400/10' },
  near: { name: 'NEAR', color: 'text-green-400 bg-green-400/10' },
  base: { name: 'Base', color: 'text-blue-300 bg-blue-300/10' },
  btc: { name: 'Bitcoin', color: 'text-orange-400 bg-orange-400/10' },
  arb: { name: 'Arbitrum', color: 'text-blue-500 bg-blue-500/10' },
  aurora: { name: 'Aurora', color: 'text-emerald-400 bg-emerald-400/10' },
  turbochain: { name: 'TurboChain', color: 'text-cyan-400 bg-cyan-400/10' },

  // Additional chains from API
  gnosis: { name: 'Gnosis', color: 'text-teal-400 bg-teal-400/10' },
  bsc: { name: 'BNB Chain', color: 'text-yellow-400 bg-yellow-400/10' },
  op: { name: 'Optimism', color: 'text-red-400 bg-red-400/10' },
  xlayer: { name: 'X Layer', color: 'text-slate-400 bg-slate-400/10' },
  pol: { name: 'Polygon', color: 'text-violet-400 bg-violet-400/10' },
  monad: { name: 'Monad', color: 'text-indigo-400 bg-indigo-400/10' },
  avax: { name: 'Avalanche', color: 'text-red-500 bg-red-500/10' },
  tron: { name: 'Tron', color: 'text-red-300 bg-red-300/10' },
  ton: { name: 'TON', color: 'text-sky-400 bg-sky-400/10' },
  sui: { name: 'Sui', color: 'text-cyan-300 bg-cyan-300/10' },
  stellar: { name: 'Stellar', color: 'text-slate-300 bg-slate-300/10' },
  plasma: { name: 'Plasma', color: 'text-pink-400 bg-pink-400/10' },
  bera: { name: 'Berachain', color: 'text-amber-400 bg-amber-400/10' },
  aptos: { name: 'Aptos', color: 'text-teal-300 bg-teal-300/10' },
  xrp: { name: 'XRP Ledger', color: 'text-slate-400 bg-slate-400/10' },
  starknet: { name: 'Starknet', color: 'text-indigo-300 bg-indigo-300/10' },
  ltc: { name: 'Litecoin', color: 'text-gray-400 bg-gray-400/10' },
  doge: { name: 'Dogecoin', color: 'text-yellow-300 bg-yellow-300/10' },
  cardano: { name: 'Cardano', color: 'text-blue-400 bg-blue-400/10' },
  bch: { name: 'Bitcoin Cash', color: 'text-green-500 bg-green-500/10' },
  adi: { name: 'ADI', color: 'text-purple-300 bg-purple-300/10' },
};

const DEFAULT_CHAIN_INFO: ChainInfo = {
  name: 'Unknown',
  color: 'text-gray-400 bg-gray-400/10',
};

/** Get chain info by blockchain ID, returns default gray styling for unknown chains */
export function getChainInfo(chain: string | undefined): ChainInfo {
  if (!chain) return DEFAULT_CHAIN_INFO;
  return CHAIN_METADATA[chain] ?? DEFAULT_CHAIN_INFO;
}

/** Get chain display name */
export function getChainName(chain: string | undefined): string {
  return getChainInfo(chain).name;
}

/** Get chain color classes */
export function getChainColor(chain: string | undefined): string {
  return getChainInfo(chain).color;
}
