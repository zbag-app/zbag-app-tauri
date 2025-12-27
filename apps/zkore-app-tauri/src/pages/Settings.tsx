import type * as IPC from '../types/ipc';
import { ViewSeedPhraseDialog } from '../components/common/ViewSeedPhraseDialog';

export function Settings(props: { wallet: IPC.WalletInfo }) {
  const { wallet } = props;

  return (
    <div style={{ display: 'grid', gap: 16 }}>
      <h1 style={{ margin: 0 }}>Settings</h1>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Security</h2>
        <div style={{ fontSize: 14, opacity: 0.85 }}>
          More security settings are coming soon.
        </div>
        <ViewSeedPhraseDialog walletId={wallet.id} triggerLabel="View seed phrase" />
      </section>
    </div>
  );
}

