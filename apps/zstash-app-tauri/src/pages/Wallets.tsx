import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Wallet, Plus, RotateCcw, RefreshCw, CheckCircle, Key } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Badge } from '../components/ui/badge';
import { listWallets, loadWallet } from '../services/ipc';

function formatTimestamp(ts: number | null): string {
  if (!ts) return '-';
  try {
    return new Date(ts).toLocaleString();
  } catch {
    return String(ts);
  }
}

export function Wallets(props: { activeWalletId: string | null; onLoaded: (resp: IPC.LoadWalletResponse) => void }) {
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
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <Wallet className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Wallets</h1>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => navigate('/create')} disabled={loading}>
            <Plus className="h-4 w-4" />
            Create
          </Button>
          <Button variant="outline" size="sm" onClick={() => navigate('/restore')} disabled={loading}>
            <RotateCcw className="h-4 w-4" />
            Restore
          </Button>
          <Button variant="outline" size="sm" onClick={refresh} disabled={loading}>
            <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
          </Button>
        </div>
      </div>

      {error && (
        <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      <div className="space-y-3">
        {wallets.length === 0 && !loading && (
          <Card>
            <CardContent className="pt-6">
              <p className="text-muted-foreground text-center">No wallets found</p>
            </CardContent>
          </Card>
        )}

        {wallets.map((w) => {
          const isActive = w.id === activeWalletId;
          return (
            <Card
              key={w.id}
              className={isActive ? 'border-primary/50 bg-primary/5' : ''}
            >
              <CardContent className="pt-6">
                <div className="flex items-start justify-between gap-4">
                  <div className="space-y-2 flex-1">
                    <div className="flex items-center gap-3 flex-wrap">
                      <h3 className="font-semibold">{w.name}</h3>
                      <Badge variant={w.network === 'Mainnet' ? 'success' : 'warning'}>
                        {w.network}
                      </Badge>
                      {w.wallet_type === 'WatchOnly' && (
                        <Badge variant="outline" className="flex items-center gap-1 text-xs">
                          <Key className="h-3 w-3" />
                          Hardware
                        </Badge>
                      )}
                      {isActive && (
                        <Badge variant="default" className="gap-1">
                          <CheckCircle className="h-3 w-3" />
                          Active
                        </Badge>
                      )}
                    </div>
                    <div className="text-xs text-muted-foreground space-y-1">
                      <div>Last opened: {formatTimestamp(w.last_opened_at)}</div>
                      <div className="font-mono">{w.id}</div>
                    </div>
                  </div>
                  <Button
                    variant={isActive ? 'secondary' : 'default'}
                    size="sm"
                    onClick={() => openWallet(w.id)}
                    disabled={loading || isActive}
                  >
                    {isActive ? 'Active' : 'Open'}
                  </Button>
                </div>
              </CardContent>
            </Card>
          );
        })}
      </div>
    </div>
  );
}
