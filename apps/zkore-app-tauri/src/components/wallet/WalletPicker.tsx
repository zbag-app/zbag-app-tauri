import { useEffect, useState } from 'react';
import { Shield, Plus, RotateCcw, RefreshCw } from 'lucide-react';
import type * as IPC from '../../types/ipc';
import { Card, CardContent } from '../ui/card';
import { Button } from '../ui/button';
import { Badge } from '../ui/badge';
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
    <div className="flex min-h-screen items-center justify-center p-4">
      <div className="w-full max-w-lg space-y-6 animate-[scale-in_0.3s_ease-out]">
        <div className="text-center space-y-2">
          <div className="mx-auto flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10">
            <Shield className="h-8 w-8 text-primary" />
          </div>
          <h1 className="font-display text-3xl font-bold">Zkore</h1>
          <p className="text-muted-foreground">Select a wallet to continue</p>
        </div>

        {error && (
          <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive text-center">
            {error}
          </div>
        )}

        <div className="space-y-3">
          {loading && wallets.length === 0 ? (
            <Card>
              <CardContent className="pt-6">
                <p className="text-center text-muted-foreground">Loading wallets...</p>
              </CardContent>
            </Card>
          ) : (
            wallets.map((w, index) => (
              <Card
                key={w.id}
                className={`cursor-pointer transition-all hover:border-primary/50 ${
                  index === 0 ? 'border-primary/30 bg-primary/5' : ''
                } ${loadingWalletId !== null ? 'pointer-events-none opacity-75' : ''}`}
                onClick={() => openWallet(w.id)}
              >
                <CardContent className="pt-6">
                  <div className="flex items-start justify-between gap-4">
                    <div className="space-y-1 flex-1">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="font-semibold">{w.name}</span>
                        <Badge variant={w.network === 'Mainnet' ? 'success' : 'warning'}>
                          {w.network}
                        </Badge>
                        {index === 0 && w.last_opened_at && (
                          <span className="text-xs text-muted-foreground">Last used</span>
                        )}
                      </div>
                      <div className="text-xs text-muted-foreground">
                        Last opened: {formatTimestamp(w.last_opened_at)}
                      </div>
                    </div>
                    {loadingWalletId === w.id && (
                      <span className="text-xs text-primary">Opening...</span>
                    )}
                  </div>
                </CardContent>
              </Card>
            ))
          )}
        </div>

        <div className="flex gap-3 justify-center pt-4 border-t border-border">
          <Button onClick={onCreateNew} disabled={loadingWalletId !== null}>
            <Plus className="h-4 w-4" />
            Create new wallet
          </Button>
          <Button variant="outline" onClick={onRestore} disabled={loadingWalletId !== null}>
            <RotateCcw className="h-4 w-4" />
            Restore from seed
          </Button>
          <Button
            variant="outline"
            size="icon"
            onClick={refresh}
            disabled={loading || loadingWalletId !== null}
          >
            <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
          </Button>
        </div>
      </div>
    </div>
  );
}
