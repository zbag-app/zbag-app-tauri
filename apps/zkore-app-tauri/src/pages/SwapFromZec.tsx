import type * as IPC from '../types/ipc';

export function SwapFromZec(props: { wallet: IPC.WalletInfo }) {
  const { wallet } = props;

  if (wallet.network !== 'Mainnet') {
    return (
      <div style={{ display: 'grid', gap: 12, maxWidth: 560 }}>
        <h1>Swap From ZEC</h1>
        <div>Swaps are only supported for Mainnet wallets in v1.</div>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gap: 12, maxWidth: 560 }}>
      <h1>Swap From ZEC</h1>
      <div>This flow is not implemented yet.</div>
    </div>
  );
}

