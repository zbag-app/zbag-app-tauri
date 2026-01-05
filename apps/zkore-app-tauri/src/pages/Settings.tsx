import { useEffect, useState } from 'react';
import type * as IPC from '../types/ipc';
import { Link } from 'react-router-dom';
import { NetworkBadge } from '../components/common/NetworkBadge';
import { TorStatusBadge } from '../components/common/TorStatusBadge';
import { ViewSeedPhraseDialog } from '../components/common/ViewSeedPhraseDialog';
import { LogoutDialog } from '../components/common/LogoutDialog';
import { getLogLocation } from '../services/ipc';

export function Settings(props: {
  wallet: IPC.WalletInfo;
  torState: IPC.TorState | null;
  onSetTorEnabled: (enabled: boolean) => void;
  onLogout: () => void;
}) {
  const { wallet, torState, onSetTorEnabled, onLogout } = props;
  const [logLocation, setLogLocation] = useState<IPC.GetLogLocationResponse | null>(null);
  const [logError, setLogError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      const res = await getLogLocation();
      if (cancelled) return;
      if ('err' in res) {
        setLogError(res.err.message);
        return;
      }
      setLogLocation(res.ok);
    }

    run();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div style={{ display: 'grid', gap: 16 }}>
      <h1 style={{ margin: 0 }}>Settings</h1>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Wallet</h2>
        <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
          <div style={{ fontSize: 14, opacity: 0.85 }}>Network</div>
          <NetworkBadge network={wallet.network} />
        </div>
        <Link to="/settings/servers">Server settings</Link>
      </section>

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
        <LogoutDialog
          walletId={wallet.id}
          triggerLabel="Logout"
          onLogout={onLogout}
        />
      </section>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Hardware Wallet</h2>
        <div style={{ fontSize: 14, opacity: 0.85 }}>
          Import a Keystone watch-only account using a UFVK.
        </div>
        <Link to="/keystone/import">Import Keystone UFVK</Link>
      </section>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Logs</h2>
        <div style={{ fontSize: 14, opacity: 0.85 }}>
          Logs are stored locally. Include the log file path when requesting support.
        </div>
        {logLocation ? (
          <div style={{ display: 'grid', gap: 6, fontSize: 13 }}>
            <div>
              Directory: <code>{logLocation.log_directory}</code>
            </div>
            <div>
              Current log: <code>{logLocation.current_log_file}</code>
            </div>
          </div>
        ) : logError ? (
          <div style={{ color: 'crimson' }}>{logError}</div>
        ) : (
          <div style={{ fontSize: 13, opacity: 0.8 }}>Loading log location…</div>
        )}
      </section>
    </div>
  );
}
