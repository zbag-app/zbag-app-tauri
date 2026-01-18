import { useCallback, useEffect, useMemo, useState } from 'react';
import { History, RefreshCw, ChevronLeft, ChevronRight, AlertCircle } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { Badge } from '../components/ui/badge';
import { listSwaps, listTransactions, reauthWallet, retryBroadcast } from '../services/ipc';
import { onSwapChanged, onTransactionChanged } from '../services/events';
import { formatRelativeTime, formatZatoshisToZec } from '../utils/zec';

export function Activity(props: { walletId: string; activeAccountId: number | null }) {
  const { walletId, activeAccountId } = props;

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [transactions, setTransactions] = useState<IPC.TransactionInfo[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [offset, setOffset] = useState(0);
  const [swaps, setSwaps] = useState<IPC.SwapInfo[]>([]);

  const [retryTxid, setRetryTxid] = useState<string | null>(null);
  const [retryPassword, setRetryPassword] = useState('');
  const [retrying, setRetrying] = useState(false);
  const [retryError, setRetryError] = useState<string | null>(null);

  const limit = 50;

  const canPagePrev = useMemo(() => offset > 0, [offset]);
  const canPageNext = useMemo(() => offset + limit < totalCount, [offset, limit, totalCount]);

  const load = useCallback(
    async (nextOffset: number) => {
      if (activeAccountId == null) return;
      setLoading(true);
      setError(null);
      const res = await listTransactions({ account_id: activeAccountId, limit, offset: nextOffset });
      setLoading(false);
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setTransactions(res.ok.transactions);
      setTotalCount(res.ok.total_count);
      setOffset(nextOffset);
    },
    [activeAccountId]
  );

  useEffect(() => {
    setRetryTxid(null);
    setRetryPassword('');
    setRetryError(null);
    setOffset(0);
    setTransactions([]);
    setTotalCount(0);
    if (activeAccountId == null) return;
    void load(0);
  }, [activeAccountId, load]);

  const loadSwaps = useCallback(async () => {
    const res = await listSwaps();
    if ('err' in res) {
      setError(res.err.message);
      return;
    }
    setSwaps(res.ok.swaps);
  }, []);

  useEffect(() => {
    void loadSwaps();
  }, [loadSwaps]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    async function run() {
      unlisten = await onTransactionChanged((event) => {
        if (cancelled) return;
        if (activeAccountId == null) return;
        if (event.transaction.account_id !== activeAccountId) return;
        void load(offset);
      });
    }

    run();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [activeAccountId, load, offset]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    async function run() {
      unlisten = await onSwapChanged((_event) => {
        if (cancelled) return;
        void loadSwaps();
      });
    }

    run();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [loadSwaps]);

  const submitRetry = async () => {
    if (!retryTxid) return;

    setRetrying(true);
    setRetryError(null);

    const reauth = await reauthWallet({ wallet_id: walletId, password: retryPassword, purpose: 'Spend' });
    if ('err' in reauth) {
      setRetrying(false);
      setRetryError(reauth.err.message);
      return;
    }

    const res = await retryBroadcast({ txid: retryTxid, reauth_token: reauth.ok.reauth_token });
    setRetrying(false);
    if ('err' in res) {
      setRetryError(res.err.message);
      return;
    }

    setRetryTxid(null);
    setRetryPassword('');
    setRetryError(null);
    void load(offset);
  };

  const getStatusBadgeVariant = (status: string) => {
    switch (status) {
      case 'Confirmed':
        return 'success';
      case 'Pending':
        return 'warning';
      case 'Failed':
        return 'destructive';
      default:
        return 'secondary';
    }
  };

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <History className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Activity</h1>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            void load(offset);
            void loadSwaps();
          }}
          disabled={loading || activeAccountId == null}
        >
          <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
          Refresh
        </Button>
      </div>

      {activeAccountId == null && (
        <div className="text-muted-foreground">No active account</div>
      )}

      {error && (
        <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {/* Swaps Section */}
      {swaps.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Swaps</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {swaps.map((s) => {
              const expired = s.deadline != null && Date.now() >= s.deadline;
              return (
                <div key={s.id} className="rounded-none border border-border p-4 space-y-2">
                  <div className="flex items-start justify-between gap-4">
                    <div className="space-y-1">
                      <div className="font-semibold">
                        {s.swap_type}: {s.input_amount} {s.input_asset} → {s.output_amount ?? '?'} {s.output_asset}
                      </div>
                      {s.deposit_address && (
                        <div className="text-xs text-muted-foreground">
                          Deposit: <code className="break-all">{s.deposit_address}</code>
                        </div>
                      )}
                      {s.deposit_memo && (
                        <div className="text-xs text-muted-foreground">
                          Memo: <code className="break-all">{s.deposit_memo}</code>
                        </div>
                      )}
                      {s.deadline && (
                        <div className="text-xs text-muted-foreground">
                          Deadline: {expired ? <span className="text-destructive">Expired</span> : new Date(s.deadline).toLocaleString()}
                        </div>
                      )}
                      {s.last_error && (
                        <div className="text-xs text-destructive">Last error: {s.last_error}</div>
                      )}
                    </div>
                    <Badge variant={s.state === 'Completed' ? 'success' : s.state === 'Failed' ? 'destructive' : 'secondary'}>
                      {s.state}
                    </Badge>
                  </div>
                </div>
              );
            })}
          </CardContent>
        </Card>
      )}

      {/* Transactions Section */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="text-lg">Transactions</CardTitle>
            <span className="text-sm text-muted-foreground">
              {transactions.length} of {totalCount}
            </span>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          {loading && transactions.length === 0 && (
            <div className="text-muted-foreground">Loading...</div>
          )}
          {!loading && transactions.length === 0 && activeAccountId != null && (
            <div className="text-muted-foreground">No transactions</div>
          )}

          {transactions.map((tx) => (
            <div key={tx.txid} className="rounded-none border border-border p-4 space-y-3">
              <div className="flex items-start justify-between gap-4">
                <div className="space-y-1 flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <code className="text-xs break-all font-mono">{tx.txid}</code>
                    <span className="text-xs text-muted-foreground">{formatRelativeTime(tx.created_at)}</span>
                  </div>
                  <div className="flex items-center gap-3 text-sm flex-wrap">
                    <span className="font-medium">{tx.tx_type}</span>
                    <span className="text-muted-foreground">Value: {formatZatoshisToZec(tx.value)} ZEC</span>
                    <span className="text-muted-foreground">Fee: {formatZatoshisToZec(tx.fee)} ZEC</span>
                  </div>
                  {tx.memo && (
                    <div className="text-sm text-muted-foreground mt-1">
                      <span className="font-medium">Memo: </span>
                      <span className="break-words">
                        {tx.memo.length > 100 ? `${tx.memo.slice(0, 100)}...` : tx.memo}
                      </span>
                    </div>
                  )}
                </div>
                <Badge variant={getStatusBadgeVariant(tx.status)}>
                  {tx.status}
                </Badge>
              </div>

              {tx.can_retry_broadcast && (
                <div className="pt-2 border-t border-border space-y-3">
                  {tx.last_error && (
                    <div className="flex items-start gap-2 text-sm text-destructive">
                      <AlertCircle className="h-4 w-4 shrink-0 mt-0.5" />
                      <span>Last error: {tx.last_error}</span>
                    </div>
                  )}

                  {retryTxid === tx.txid ? (
                    <div className="space-y-3 max-w-md">
                      <div className="space-y-2">
                        <Label htmlFor="retry-password">Password</Label>
                        <Input
                          id="retry-password"
                          type="password"
                          value={retryPassword}
                          onChange={(e) => setRetryPassword(e.currentTarget.value)}
                          disabled={retrying}
                        />
                      </div>
                      {retryError && (
                        <div className="text-sm text-destructive">{retryError}</div>
                      )}
                      <div className="flex gap-2">
                        <Button onClick={submitRetry} disabled={!retryPassword || retrying} size="sm">
                          {retrying ? 'Retrying...' : 'Retry broadcast'}
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => {
                            setRetryTxid(null);
                            setRetryPassword('');
                            setRetryError(null);
                          }}
                          disabled={retrying}
                        >
                          Cancel
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => {
                        setRetryTxid(tx.txid);
                        setRetryPassword('');
                        setRetryError(null);
                      }}
                    >
                      Retry broadcast
                    </Button>
                  )}
                </div>
              )}
            </div>
          ))}

          {/* Pagination */}
          {(canPagePrev || canPageNext) && (
            <div className="flex items-center justify-center gap-2 pt-4">
              <Button
                variant="outline"
                size="sm"
                disabled={!canPagePrev || loading}
                onClick={() => void load(Math.max(0, offset - limit))}
              >
                <ChevronLeft className="h-4 w-4" />
                Prev
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={!canPageNext || loading}
                onClick={() => void load(offset + limit)}
              >
                Next
                <ChevronRight className="h-4 w-4" />
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
