import type * as IPC from '../types/ipc';
import { Link } from 'react-router-dom';
import { TorStatusBadge } from '../components/common/TorStatusBadge';
import { ViewSeedPhraseDialog } from '../components/common/ViewSeedPhraseDialog';

export function Settings(props: {
  wallet: IPC.WalletInfo;
  torState: IPC.TorState | null;
  onSetTorEnabled: (enabled: boolean) => void;
}) {
  const { wallet, torState, onSetTorEnabled } = props;

  return (
    <div style={{ display: 'grid', gap: 16 }}>
      <h1 style={{ margin: 0 }}>Settings</h1>

      <section style={{ display: 'grid', gap: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12 }}>
          <h2 style={{ margin: 0 }}>Tor</h2>
          <TorStatusBadge state={torState} />
        </div>
        <div style={{ fontSize: 14, opacity: 0.85 }}>
          Opt-in Tor anonymization for all network traffic. When enabled, Zkore fails closed if Tor is not healthy.
        </div>

        <label style={{ display: 'inline-flex', gap: 8, alignItems: 'center' }}>
          <input
            type="checkbox"
            checked={torState?.enabled ?? false}
            onChange={(e) => onSetTorEnabled(e.currentTarget.checked)}
            aria-label="Enable Tor"
          />
          Enable Tor (beta)
        </label>

        {torState?.enabled && torState.status !== 'On' ? (
          <div style={{ fontSize: 13, color: '#b45309' }}>
            Status: {torState.status}. Some operations may be blocked until Tor is On.
          </div>
        ) : null}
      </section>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Security</h2>
        <div style={{ fontSize: 14, opacity: 0.85 }}>
          More security settings are coming soon.
        </div>
        <ViewSeedPhraseDialog walletId={wallet.id} triggerLabel="View seed phrase" />
      </section>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Hardware Wallet</h2>
        <div style={{ fontSize: 14, opacity: 0.85 }}>
          Import a Keystone watch-only account using a UFVK.
        </div>
        <Link to="/keystone/import">Import Keystone UFVK</Link>
      </section>
    </div>
  );
}
