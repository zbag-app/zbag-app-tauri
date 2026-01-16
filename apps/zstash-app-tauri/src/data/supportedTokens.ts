import type * as IPC from '../types/ipc';

export type SupportedToken = IPC.SupportedToken;

/** ZEC asset ID */
export const ZEC_ASSET_ID = 'nep141:zec.omft.near';

/** Default non-ZEC asset for swaps */
export const DEFAULT_NON_ZEC_ASSET_ID = 'nep141:wrap.near';

/**
 * Fallback static list of supported tokens.
 * Used when the API call fails or is unavailable.
 */
export const FALLBACK_TOKENS: SupportedToken[] = [
  // Zcash
  { asset_id: 'nep141:zec.omft.near', symbol: 'ZEC', chain: 'zec', decimals: 8, usd_price: null, icon: null },
  // NEAR
  { asset_id: 'nep141:wrap.near', symbol: 'NEAR', chain: 'near', decimals: 24, usd_price: null, icon: null },
  {
    asset_id: 'nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1',
    symbol: 'USDC',
    chain: 'near',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  // Ethereum
  { asset_id: 'nep141:eth.omft.near', symbol: 'ETH', chain: 'eth', decimals: 18, usd_price: null, icon: null },
  {
    asset_id: 'nep141:eth-0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48.omft.near',
    symbol: 'USDC',
    chain: 'eth',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  {
    asset_id: 'nep141:eth-0xdac17f958d2ee523a2206206994597c13d831ec7.omft.near',
    symbol: 'USDT',
    chain: 'eth',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  // Solana
  { asset_id: 'nep141:sol.omft.near', symbol: 'SOL', chain: 'sol', decimals: 9, usd_price: null, icon: null },
  {
    asset_id: 'nep141:sol-5ce3bf3a31af18be40ba30f721101b4341690186.omft.near',
    symbol: 'USDC',
    chain: 'sol',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  {
    asset_id: 'nep141:sol-c800a4bd850783ccb82c2b2c7e84175443606352.omft.near',
    symbol: 'USDT',
    chain: 'sol',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  // Base
  {
    asset_id: 'nep141:base.omft.near',
    symbol: 'ETH',
    chain: 'base',
    decimals: 18,
    usd_price: null,
    icon: null,
  },
  {
    asset_id: 'nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near',
    symbol: 'USDC',
    chain: 'base',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
];

/** Known chain display names */
const CHAIN_NAMES: Record<string, string> = {
  near: 'NEAR',
  eth: 'Ethereum',
  sol: 'Solana',
  base: 'Base',
  zec: 'Zcash',
  btc: 'Bitcoin',
  arb: 'Arbitrum',
  aurora: 'Aurora',
  turbochain: 'TurboChain',
};

/**
 * Get a human-readable display label for a token.
 * Format: "SYMBOL (Chain)" for non-native tokens, just "SYMBOL" for native tokens.
 */
export function getTokenLabel(token: SupportedToken): string {
  const chainName = CHAIN_NAMES[token.chain] ?? token.chain.toUpperCase();

  // For native tokens (symbol matches chain), just show symbol
  if (token.symbol.toLowerCase() === token.chain.toLowerCase()) {
    return token.symbol;
  }

  // For wrapped ETH on other chains
  if (token.symbol === 'ETH' && token.chain !== 'eth') {
    return `ETH (${chainName})`;
  }

  // For stablecoins and other tokens, show chain
  return `${token.symbol} (${chainName})`;
}

/**
 * Filter tokens for swap selection.
 * Excludes ZEC (since that's the other side of swaps) and optionally filters by USD price.
 */
export function filterSwapTokens(
  tokens: SupportedToken[],
  options?: { excludeZeroPriceTokens?: boolean }
): SupportedToken[] {
  return tokens.filter((t) => {
    // Always exclude ZEC from selection (it's the swap target/source)
    if (t.asset_id === ZEC_ASSET_ID) return false;

    // Optionally filter out tokens without USD price
    if (options?.excludeZeroPriceTokens && (t.usd_price == null || t.usd_price === 0)) {
      return false;
    }

    return true;
  });
}

/**
 * Sort tokens by USD price (highest first) for better UX.
 * Tokens without price go to the end.
 */
export function sortTokensByPrice(tokens: SupportedToken[]): SupportedToken[] {
  return [...tokens].sort((a, b) => {
    const priceA = a.usd_price ?? 0;
    const priceB = b.usd_price ?? 0;
    return priceB - priceA;
  });
}

/**
 * Get tokens that can be swapped to ZEC (excludes ZEC itself).
 * @deprecated Use the dynamic tokens from getSupportedTokens() IPC call instead.
 */
export function getToZecTokens(): SupportedToken[] {
  return filterSwapTokens(FALLBACK_TOKENS);
}

/**
 * Get tokens that ZEC can be swapped to (excludes ZEC itself).
 * @deprecated Use the dynamic tokens from getSupportedTokens() IPC call instead.
 */
export function getFromZecTokens(): SupportedToken[] {
  return filterSwapTokens(FALLBACK_TOKENS);
}
