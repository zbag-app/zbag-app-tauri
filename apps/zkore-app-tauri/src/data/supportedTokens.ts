export type SupportedToken = {
  id: string;
  label: string;
};

export const supportedTokens: SupportedToken[] = [
  { id: 'near:mainnet:native', label: 'NEAR' },
  { id: 'usdc:mainnet:near', label: 'USDC (NEAR)' },
  { id: 'zcash:mainnet:native', label: 'ZEC' },
];

