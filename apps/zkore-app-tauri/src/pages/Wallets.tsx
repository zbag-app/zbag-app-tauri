import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';
import { NetworkBadge } from '../components/common/NetworkBadge';
import { listWallets, loadWallet } from '../services/ipc';

function formatTimestamp(ts: number | null): string {
  if (!ts) return '—';
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return String(ts);
  }
}

export function Wallets(props: { activeWalletId: string; onLoaded: (resp: IPC.LoadWalletResponse) => void }) {
  const { activeWalletId, onLoaded } = props;
  const navigate = useNavigate();

  const [wallets, setWallets] = useState<IPC.WalletInfo[]>([]);
  const [loading, setLoading] = useState(false);
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
      setWallets(res.ok.wallets);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const openWallet = async (walletId: string) => {
    setLoading(true);
    setError(null);
    try {
      const res = await loadWallet({ wallet_id: walletId });
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      onLoaded(res.ok);
      navigate('/');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ display: 'grid', gap: 14 }}>
      <header style={{ display: 'flex', gap: 12, alignItems: 'baseline', flexWrap: 'wrap' }}>
        <h1 style={{ margin: 0 }}>Wallets</h1>
        <button type="button" onClick={() => navigate('/create')} disabled={loading}>
          Create
        </button>
        <button type="button" onClick={() => navigate('/restore')} disabled={loading}>
          Restore
        </button>
        <button type="button" onClick={refresh} disabled={loading}>
          Refresh
        </button>
      </header>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'grid', gap: 8 }}>
        {wallets.length === 0 ? <div>No wallets found.</div> : null}
        {wallets.map((w) => {
          const isActive = w.id === activeWalletId;
          return (
            <div
              key={w.id}
              style={{
                border: '1px solid #e5e7eb',
                borderRadius: 12,
                padding: 12,
                display: 'grid',
                gap: 6,
                background: isActive ? '#f8fafc' : 'white',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
                  <strong>{w.name}</strong>
                  <NetworkBadge network={w.network} />
                  {isActive ? (
                    <span style={{ fontSize: 12, opacity: 0.8 }}>
                      Active
                    </span>
                  ) : null}
                </div>
                <button type="button" onClick={() => openWallet(w.id)} disabled={loading || isActive}>
                  {isActive ? 'Open' : 'Switch'}
                </button>
              </div>

              <div style={{ fontSize: 12, opacity: 0.8 }}>
                Last opened: {formatTimestamp(w.last_opened_at)}
              </div>
              <div style={{ fontSize: 12, opacity: 0.8 }}>Wallet ID: {w.id}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

