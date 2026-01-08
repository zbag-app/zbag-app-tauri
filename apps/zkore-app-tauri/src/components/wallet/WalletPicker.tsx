import { useEffect, useState } from 'react';
import type * as IPC from '../../types/ipc';
import { NetworkBadge } from '../common/NetworkBadge';
import { listWallets, loadWallet } from '../../services/ipc';

function formatTimestamp(ts: number | null): string {
  if (!ts) return 'Never';
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return String(ts);
  }
}

function sortWalletsByRecent(wallets: IPC.WalletInfo[]): IPC.WalletInfo[] {
  return wallets.slice().sort((a, b) => {
    const aT = a.last_opened_at ?? a.created_at;
    const bT = b.last_opened_at ?? b.created_at;
    return bT - aT;
  });
}

export function WalletPicker(props: {
  onLoaded: (resp: IPC.LoadWalletResponse) => void;
  onCreateNew: () => void;
  onRestore: () => void;
}) {
  const { onLoaded, onCreateNew, onRestore } = props;

  const [wallets, setWallets] = useState<IPC.WalletInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadingWalletId, setLoadingWalletId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await listWallets();
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setWallets(sortWalletsByRecent(res.ok.wallets));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const openWallet = async (walletId: string) => {
    setLoadingWalletId(walletId);
    setError(null);
    try {
      const res = await loadWallet({ wallet_id: walletId });
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      onLoaded(res.ok);
    } finally {
      setLoadingWalletId(null);
    }
  };

  return (
    <div
      style={{
        display: 'grid',
        gap: 24,
        padding: 32,
        maxWidth: 560,
        margin: '0 auto',
        minHeight: '100vh',
        alignContent: 'center',
      }}
    >
      <header style={{ textAlign: 'center' }}>
        <h1 style={{ margin: 0, marginBottom: 8 }}>Zkore</h1>
        <p style={{ margin: 0, opacity: 0.7 }}>Select a wallet to continue</p>
      </header>

      {error ? <div style={{ color: 'crimson', textAlign: 'center' }}>{error}</div> : null}

      <div style={{ display: 'grid', gap: 10 }}>
        {loading && wallets.length === 0 ? (
          <div style={{ textAlign: 'center', opacity: 0.7 }}>Loading wallets...</div>
        ) : (
          wallets.map((w, index) => (
            <button
              key={w.id}
              type="button"
              onClick={() => openWallet(w.id)}
              disabled={loadingWalletId !== null}
              style={{
                border: '1px solid #e5e7eb',
                borderRadius: 12,
                padding: 14,
                display: 'grid',
                gap: 6,
                background: index === 0 ? '#f8fafc' : 'white',
                cursor: loadingWalletId !== null ? 'wait' : 'pointer',
                textAlign: 'left',
              }}
            >
              <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
                <strong>{w.name}</strong>
                <NetworkBadge network={w.network} />
                {index === 0 && w.last_opened_at ? (
                  <span style={{ fontSize: 12, opacity: 0.6 }}>Last used</span>
                ) : null}
              </div>
              <div style={{ fontSize: 12, opacity: 0.7 }}>
                Last opened: {formatTimestamp(w.last_opened_at)}
              </div>
              {loadingWalletId === w.id ? (
                <div style={{ fontSize: 12, color: '#3b82f6' }}>Opening...</div>
              ) : null}
            </button>
          ))
        )}
      </div>

      <div
        style={{
          display: 'flex',
          gap: 12,
          justifyContent: 'center',
          borderTop: '1px solid #e5e7eb',
          paddingTop: 24,
          marginTop: 8,
        }}
      >
        <button type="button" onClick={onCreateNew} disabled={loadingWalletId !== null}>
          Create new wallet
        </button>
        <button type="button" onClick={onRestore} disabled={loadingWalletId !== null}>
          Restore from seed
        </button>
        <button
          type="button"
          onClick={refresh}
          disabled={loading || loadingWalletId !== null}
        >
          Refresh
        </button>
      </div>
    </div>
  );
}
