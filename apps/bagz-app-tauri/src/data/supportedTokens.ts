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
  // === Zcash ===
  { asset_id: 'nep141:zec.omft.near', symbol: 'ZEC', chain: 'zec', decimals: 8, usd_price: null, icon: null },

  // === Bitcoin ===
  { asset_id: 'nep141:btc.omft.near', symbol: 'BTC', chain: 'btc', decimals: 8, usd_price: null, icon: null },

  // === Ethereum ===
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

  // === Solana ===
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

  // === NEAR ===
  { asset_id: 'nep141:wrap.near', symbol: 'NEAR', chain: 'near', decimals: 24, usd_price: null, icon: null },
  {
    asset_id: 'nep141:17208628f84f5d6ad33f0da3bbbeb27ffcb398eac501a31bd6ad2011e36133a1',
    symbol: 'USDC',
    chain: 'near',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },

  // === BNB Chain (BSC) ===
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:56_11111111111111111111',
    symbol: 'BNB',
    chain: 'bsc',
    decimals: 18,
    usd_price: null,
    icon: null,
  },
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:56_2w93GqMcEmQFDru84j3HZZWt557r',
    symbol: 'USDC',
    chain: 'bsc',
    decimals: 18,
    usd_price: 1.0,
    icon: null,
  },
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:56_2CMMyVTGZkeyNZTSvS5sarzfir6g',
    symbol: 'USDT',
    chain: 'bsc',
    decimals: 18,
    usd_price: 1.0,
    icon: null,
  },

  // === Avalanche ===
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:43114_11111111111111111111',
    symbol: 'AVAX',
    chain: 'avax',
    decimals: 18,
    usd_price: null,
    icon: null,
  },
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:43114_3atVJH3r5c4GqiSYmg9fECvjc47o',
    symbol: 'USDC',
    chain: 'avax',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:43114_372BeH7ENZieCaabwkbWkBiTTgXf',
    symbol: 'USDT',
    chain: 'avax',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },

  // === Polygon ===
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:137_11111111111111111111',
    symbol: 'POL',
    chain: 'pol',
    decimals: 18,
    usd_price: null,
    icon: null,
  },
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:137_qiStmoQJDQPTebaPjgx5VBxZv6L',
    symbol: 'USDC',
    chain: 'pol',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:137_3hpYoaLtt8MP1Z2GH1U473DMRKgr',
    symbol: 'USDT',
    chain: 'pol',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },

  // === Arbitrum ===
  { asset_id: 'nep141:arb.omft.near', symbol: 'ETH', chain: 'arb', decimals: 18, usd_price: null, icon: null },
  {
    asset_id: 'nep141:arb-0xaf88d065e77c8cc2239327c5edb3a432268e5831.omft.near',
    symbol: 'USDC',
    chain: 'arb',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  {
    asset_id: 'nep141:arb-0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9.omft.near',
    symbol: 'USDT',
    chain: 'arb',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },

  // === Optimism ===
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:10_11111111111111111111',
    symbol: 'ETH',
    chain: 'op',
    decimals: 18,
    usd_price: null,
    icon: null,
  },
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:10_A2ewyUyDp6qsue1jqZsGypkCxRJ',
    symbol: 'USDC',
    chain: 'op',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:10_359RPSJVdTxwTJT9TyGssr2rFoWo',
    symbol: 'USDT',
    chain: 'op',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },

  // === Base ===
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

  // === TON ===
  {
    asset_id: 'nep245:v2_1.omni.hot.tg:1117_',
    symbol: 'TON',
    chain: 'ton',
    decimals: 9,
    usd_price: null,
    icon: null,
  },

  // === Dogecoin ===
  { asset_id: 'nep141:doge.omft.near', symbol: 'DOGE', chain: 'doge', decimals: 8, usd_price: null, icon: null },

  // === Litecoin ===
  { asset_id: 'nep141:ltc.omft.near', symbol: 'LTC', chain: 'ltc', decimals: 8, usd_price: null, icon: null },

  // === XRP ===
  { asset_id: 'nep141:xrp.omft.near', symbol: 'XRP', chain: 'xrp', decimals: 6, usd_price: null, icon: null },

  // === Tron ===
  { asset_id: 'nep141:tron.omft.near', symbol: 'TRX', chain: 'tron', decimals: 6, usd_price: null, icon: null },
  {
    asset_id: 'nep141:tron-d28a265909efecdcee7c5028585214ea0b96f015.omft.near',
    symbol: 'USDT',
    chain: 'tron',
    decimals: 6,
    usd_price: 1.0,
    icon: null,
  },

  // === Sui ===
  { asset_id: 'nep141:sui.omft.near', symbol: 'SUI', chain: 'sui', decimals: 9, usd_price: null, icon: null },

  // === Gnosis ===
  { asset_id: 'nep141:gnosis.omft.near', symbol: 'xDAI', chain: 'gnosis', decimals: 18, usd_price: null, icon: null },
];

/** Known chain display names */
const CHAIN_NAMES: Record<string, string> = {
  zec: 'Zcash',
  btc: 'Bitcoin',
  eth: 'Ethereum',
  sol: 'Solana',
  near: 'NEAR',
  bsc: 'BNB Chain',
  avax: 'Avalanche',
  pol: 'Polygon',
  arb: 'Arbitrum',
  op: 'Optimism',
  base: 'Base',
  ton: 'TON',
  doge: 'Dogecoin',
  ltc: 'Litecoin',
  xrp: 'XRP Ledger',
  tron: 'Tron',
  sui: 'Sui',
  gnosis: 'Gnosis',
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

/** Token info for formatting display */
export type TokenDisplayInfo = {
  decimals: number;
  label: string;
};

/** Static lookup map for fallback tokens */
const fallbackTokensById = new Map<string, SupportedToken>(
  FALLBACK_TOKENS.map((t) => [t.asset_id, t])
);

/**
 * Look up a token by its asset ID and return display info.
 * Falls back to static token list for offline/error scenarios.
 */
export function getTokenById(assetId: string): TokenDisplayInfo | undefined {
  const token = fallbackTokensById.get(assetId);
  if (!token) return undefined;
  return {
    decimals: token.decimals,
    label: getTokenLabel(token),
  };
}
