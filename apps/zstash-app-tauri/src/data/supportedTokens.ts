export type SupportedToken = {
  id: string;
  label: string;
  decimals: number;
  blockchain?: string;
};

export const supportedTokens: SupportedToken[] = [
  // Zcash
  { id: 'nep141:zec.omft.near', label: 'ZEC', decimals: 8, blockchain: 'zec' },
  // NEAR
  { id: 'nep141:wrap.near', label: 'NEAR', decimals: 24, blockchain: 'near' },
  {
    id: 'nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1',
    label: 'USDC (NEAR)',
    decimals: 6,
    blockchain: 'near',
  },
  // Ethereum
  { id: 'nep141:eth.omft.near', label: 'ETH', decimals: 18, blockchain: 'eth' },
  {
    id: 'nep141:eth-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48.omft.near',
    label: 'USDC (ETH)',
    decimals: 6,
    blockchain: 'eth',
  },
  {
    id: 'nep141:eth-0xdac17f958d2ee523a2206206994597c13d831ec7.omft.near',
    label: 'USDT (ETH)',
    decimals: 6,
    blockchain: 'eth',
  },
  // Solana
  { id: 'nep141:sol.omft.near', label: 'SOL', decimals: 9, blockchain: 'sol' },
  {
    id: 'nep141:sol-5ce3bf3a31af18be40ba30f721101b4341690186.omft.near',
    label: 'USDC (SOL)',
    decimals: 6,
    blockchain: 'sol',
  },
  {
    id: 'nep141:sol-c800a4bd850783ccb82c2b2c7e84175443606352.omft.near',
    label: 'USDT (SOL)',
    decimals: 6,
    blockchain: 'sol',
  },
  // Base
  {
    id: 'nep141:base.omft.near',
    label: 'ETH (Base)',
    decimals: 18,
    blockchain: 'base',
  },
  {
    id: 'nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near',
    label: 'USDC (Base)',
    decimals: 6,
    blockchain: 'base',
  },
];

/** Get the ZEC asset ID */
export const ZEC_ASSET_ID = 'nep141:zec.omft.near';

/** Get default non-ZEC asset for swaps */
export const DEFAULT_NON_ZEC_ASSET_ID = 'nep141:wrap.near';

/** Get tokens that can be swapped to ZEC (excludes ZEC itself) */
export function getToZecTokens(): SupportedToken[] {
  return supportedTokens.filter((t) => t.id !== ZEC_ASSET_ID);
}

/** Get tokens that ZEC can be swapped to (excludes ZEC itself) */
export function getFromZecTokens(): SupportedToken[] {
  return supportedTokens.filter((t) => t.id !== ZEC_ASSET_ID);
}
