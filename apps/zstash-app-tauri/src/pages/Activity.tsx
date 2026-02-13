import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { History, ChevronLeft, ChevronRight, AlertCircle, Copy, ChevronDown, ChevronUp, Check, FileText } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { Badge } from '../components/ui/badge';
import { listSwaps, listTransactions, reauthWallet, retryBroadcast } from '../services/ipc';
import { onSwapChanged, onSyncProgress, onTransactionChanged } from '../services/events';
import { formatRelativeTime, formatZatoshisToZec, formatFiat, zatoshisToFiat } from '../utils/zec';
import { useFiatDisplayContext } from '../context/FiatDisplayContext';
import { getDisplayableMemos, getMemoDisplayText } from '../utils/memo';

interface MemoDisplayProps {
  tx: IPC.TransactionInfo;
  isExpanded: boolean;
  onToggleExpanded: () => void;
  copiedMemo: string | null;
  copyError: string | null;
  onCopyMemo: (txid: string, content: string) => void;
}

const SYNC_REFRESH_MIN_INTERVAL_MS = 1500;
const SYNC_REFRESH_BLOCK_THRESHOLD = 250;
const SYNC_NEAR_TIP_LAG_BLOCKS = 1;
type LiveSubscriptionKey = 'sync' | 'transaction' | 'swap';

const LIVE_SUBSCRIPTION_LABELS: Record<LiveSubscriptionKey, string> = {
  sync: 'sync progress',
  transaction: 'transaction updates',
  swap: 'swap updates',
};

const EMPTY_SUBSCRIPTION_ERRORS: Record<LiveSubscriptionKey, string | null> = {
  sync: null,
  transaction: null,
  swap: null,
};

function MemoDisplay({ tx, isExpanded, onToggleExpanded, copiedMemo, copyError, onCopyMemo }: MemoDisplayProps) {
  const totalMemos = tx.memo_count;

  const { displayableMemos, fullText, fullTextBytes, hiddenCount } = useMemo(() => {
    const displayable = getDisplayableMemos(tx.memos);
    const text = displayable.length > 0 ? getMemoDisplayText(displayable) : '';
    const textBytes = new TextEncoder().encode(text).length;
    const hidden = totalMemos - displayable.length;
    return { displayableMemos: displayable, fullText: text, fullTextBytes: textBytes, hiddenCount: hidden };
  }, [tx.memos, totalMemos]);

  if (totalMemos === 0) return null;

  const isLong = fullText.length > 100;
  const displayText = isExpanded || !isLong ? fullText : fullText.slice(0, 100);

  return (
    <div className="mt-2 rounded-md border border-border/50 bg-muted/30 p-3">
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 text-sm font-medium text-foreground">
          <FileText className="h-4 w-4 shrink-0" />
          <span>Memo{totalMemos !== 1 ? `s (${totalMemos})` : ''}</span>
          {hiddenCount > 0 && displayableMemos.length > 0 && (
            <span className="text-xs text-muted-foreground">({displayableMemos.length} shown)</span>
          )}
        </div>
        {displayableMemos.length > 0 && (
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="sm"
              className="h-7 w-7 p-0"
              onClick={() => onCopyMemo(tx.txid, fullText)}
              title={copyError === tx.txid ? 'Copy failed' : 'Copy memo'}
            >
              {copiedMemo === tx.txid ? (
                <Check className="h-3.5 w-3.5 text-green-500" />
              ) : copyError === tx.txid ? (
                <AlertCircle className="h-3.5 w-3.5 text-destructive" />
              ) : (
                <Copy className="h-3.5 w-3.5" />
              )}
            </Button>
          </div>
        )}
      </div>
      {displayableMemos.length > 0 && (
        <>
          <div className="mt-2 text-sm text-muted-foreground whitespace-pre-wrap break-words">
            {displayText}
            {isLong && !isExpanded && '...'}
          </div>
          {isLong && (
            <Button
              variant="ghost"
              size="sm"
              className="mt-2 h-7 px-2 text-xs"
              onClick={onToggleExpanded}
            >
              {isExpanded ? (
                <>
                  <ChevronUp className="h-3.5 w-3.5 mr-1" />
                  Show less
                </>
              ) : (
                <>
                  <ChevronDown className="h-3.5 w-3.5 mr-1" />
                  Show more ({fullTextBytes} bytes)
                </>
              )}
            </Button>
          )}
        </>
      )}
      {displayableMemos.some((m) => m.kind === 'Binary') && (
        <div className="mt-2 text-xs text-muted-foreground/70">
          Contains binary data that cannot be displayed as text
        </div>
      )}
    </div>
  );
}

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
  const [expandedMemos, setExpandedMemos] = useState<Set<string>>(new Set());
  const [copiedMemo, setCopiedMemo] = useState<string | null>(null);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [subscriptionErrors, setSubscriptionErrors] = useState<
    Record<LiveSubscriptionKey, string | null>
  >({ ...EMPTY_SUBSCRIPTION_ERRORS });
  const [subscriptionRetryToken, setSubscriptionRetryToken] = useState(0);
  const lastSyncFrontierRef = useRef<number | null>(null);
  const lastSyncRefreshAtRef = useRef(0);
  const frontierAtLastSyncRefreshRef = useRef<number | null>(null);
  const syncRefreshInFlightRef = useRef(false);
  const offsetRef = useRef(0);

  // Use centralized fiat display context
  const { settings: fiatSettings, rate: exchangeRate, isStale: fiatIsStale } = useFiatDisplayContext();

  const limit = 50;

  const canPagePrev = useMemo(() => offset > 0, [offset]);
  const canPageNext = useMemo(() => offset + limit < totalCount, [offset, limit, totalCount]);
  const hasPendingTransactions = useMemo(
    () => transactions.some((tx) => tx.status === 'Pending'),
    [transactions]
  );
  const failedSubscriptionLabels = useMemo(
    () =>
      (Object.entries(subscriptionErrors) as [LiveSubscriptionKey, string | null][])
        .filter(([, message]) => message != null)
        .map(([key]) => LIVE_SUBSCRIPTION_LABELS[key]),
    [subscriptionErrors]
  );
  const hasSubscriptionFailure = failedSubscriptionLabels.length > 0;

  const load = useCallback(
    async (nextOffset: number) => {
      if (activeAccountId == null) return;
      setLoading(true);
      setError(null);
      try {
        const res = await listTransactions({ account_id: activeAccountId, limit, offset: nextOffset });
        if ('err' in res) {
          setError(res.err.message);
          return;
        }
        setTransactions(res.ok.transactions);
        setTotalCount(res.ok.total_count);
        setOffset(nextOffset);
        offsetRef.current = nextOffset;
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Failed to load transactions';
        setError(message);
      } finally {
        setLoading(false);
      }
    },
    [activeAccountId]
  );

  useEffect(() => {
    setRetryTxid(null);
    setRetryPassword('');
    setRetryError(null);
    setSubscriptionErrors({ ...EMPTY_SUBSCRIPTION_ERRORS });
    setOffset(0);
    offsetRef.current = 0;
    setTransactions([]);
    setTotalCount(0);
    lastSyncFrontierRef.current = null;
    lastSyncRefreshAtRef.current = 0;
    frontierAtLastSyncRefreshRef.current = null;
    syncRefreshInFlightRef.current = false;
    if (activeAccountId == null) return;
    void load(0);
  }, [activeAccountId, load]);

  const loadSwaps = useCallback(async () => {
    try {
      const res = await listSwaps();
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setSwaps(res.ok.swaps);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load swaps';
      setError(message);
    }
  }, []);

  useEffect(() => {
    void loadSwaps();
  }, [loadSwaps]);

  const triggerSyncRefresh = useCallback(() => {
    if (syncRefreshInFlightRef.current) return;
    syncRefreshInFlightRef.current = true;
    void load(offsetRef.current).finally(() => {
      syncRefreshInFlightRef.current = false;
    });
  }, [load]);

  const markSubscriptionHealthy = useCallback((key: LiveSubscriptionKey) => {
    setSubscriptionErrors((prev) => (prev[key] == null ? prev : { ...prev, [key]: null }));
  }, []);

  const markSubscriptionFailed = useCallback((key: LiveSubscriptionKey, err: unknown) => {
    const message = err instanceof Error ? err.message : 'Unknown subscription error';
    setSubscriptionErrors((prev) => ({ ...prev, [key]: message }));
  }, []);

  const refreshActivityData = useCallback(() => {
    if (activeAccountId != null) {
      void load(offsetRef.current);
    }
    void loadSwaps();
  }, [activeAccountId, load, loadSwaps]);

  const retryLiveUpdates = useCallback(() => {
    setSubscriptionErrors({ ...EMPTY_SUBSCRIPTION_ERRORS });
    setSubscriptionRetryToken((prev) => prev + 1);
    refreshActivityData();
  }, [refreshActivityData]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    onSyncProgress((event) => {
        if (cancelled) return;
        if (activeAccountId == null) return;
        if (!hasPendingTransactions) return;

        const frontier = event.progress.scan_frontier_height;
        const tip = event.progress.wallet_tip_height;
        const previousFrontier = lastSyncFrontierRef.current;
        lastSyncFrontierRef.current = frontier;

        if (previousFrontier != null && frontier <= previousFrontier) return;

        const lag = tip > frontier ? tip - frontier : 0;
        const nearTip = tip > 0 && lag <= SYNC_NEAR_TIP_LAG_BLOCKS;
        const now = Date.now();

        if (nearTip) {
          lastSyncRefreshAtRef.current = now;
          frontierAtLastSyncRefreshRef.current = frontier;
          triggerSyncRefresh();
          return;
        }

        const elapsedMs =
          lastSyncRefreshAtRef.current > 0
            ? now - lastSyncRefreshAtRef.current
            : Number.POSITIVE_INFINITY;
        const scannedSinceLastRefresh =
          frontierAtLastSyncRefreshRef.current != null
            ? frontier - frontierAtLastSyncRefreshRef.current
            : Number.POSITIVE_INFINITY;

        if (
          elapsedMs >= SYNC_REFRESH_MIN_INTERVAL_MS ||
          scannedSinceLastRefresh >= SYNC_REFRESH_BLOCK_THRESHOLD
        ) {
          lastSyncRefreshAtRef.current = now;
          frontierAtLastSyncRefreshRef.current = frontier;
          triggerSyncRefresh();
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
        markSubscriptionHealthy('sync');
      })
      .catch((err) => {
        if (cancelled) return;
        console.warn('Failed to subscribe to sync progress events:', err);
        markSubscriptionFailed('sync', err);
      });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [
    activeAccountId,
    hasPendingTransactions,
    triggerSyncRefresh,
    markSubscriptionHealthy,
    markSubscriptionFailed,
    subscriptionRetryToken,
  ]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    onTransactionChanged((event) => {
        if (cancelled) return;
        if (activeAccountId == null) return;
        if (event.transaction.account_id !== activeAccountId) return;
        void load(offsetRef.current);
      })
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
        markSubscriptionHealthy('transaction');
      })
      .catch((err) => {
        if (cancelled) return;
        console.warn('Failed to subscribe to transaction events:', err);
        markSubscriptionFailed('transaction', err);
      });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [
    activeAccountId,
    load,
    markSubscriptionHealthy,
    markSubscriptionFailed,
    subscriptionRetryToken,
  ]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    onSwapChanged((_event) => {
        if (cancelled) return;
        void loadSwaps();
      })
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
        markSubscriptionHealthy('swap');
      })
      .catch((err) => {
        if (cancelled) return;
        console.warn('Failed to subscribe to swap events:', err);
        markSubscriptionFailed('swap', err);
      });
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [loadSwaps, markSubscriptionHealthy, markSubscriptionFailed, subscriptionRetryToken]);

  const submitRetry = async () => {
    if (!retryTxid) return;

    setRetrying(true);
    setRetryError(null);

    try {
      const reauth = await reauthWallet({ wallet_id: walletId, password: retryPassword, purpose: 'Spend' });
      if ('err' in reauth) {
        setRetryError(reauth.err.message);
        return;
      }

      const res = await retryBroadcast({ txid: retryTxid, reauth_token: reauth.ok.reauth_token });
      if ('err' in res) {
        setRetryError(res.err.message);
        return;
      }

      setRetryTxid(null);
      setRetryPassword('');
      setRetryError(null);
      void load(offsetRef.current);
    } catch (err) {
      setRetryError(err instanceof Error ? err.message : 'Failed to retry broadcast');
    } finally {
      setRetrying(false);
    }
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

  const toggleMemoExpanded = (txid: string) => {
    setExpandedMemos((prev) => {
      const next = new Set(prev);
      if (next.has(txid)) {
        next.delete(txid);
      } else {
        next.add(txid);
      }
      return next;
    });
  };

  const copyMemo = async (txid: string, content: string) => {
    try {
      await navigator.clipboard.writeText(content);
      setCopiedMemo(txid);
      setCopyError(null);
      setTimeout(() => setCopiedMemo(null), 2000);
    } catch (err) {
      // Log error for debugging (clipboard may be unavailable in sandboxed environments)
      console.error("Failed to copy memo to clipboard:", err);
      setCopyError(txid);
      setTimeout(() => setCopyError(null), 2000);
    }
  };

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <History className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Activity</h1>
        </div>
      </div>

      {activeAccountId == null && (
        <div className="text-muted-foreground">No active account</div>
      )}

      {error && (
        <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {hasSubscriptionFailure && (
        <div className="rounded-none border border-amber-500/40 bg-amber-500/10 p-3 text-sm text-amber-700">
          <div className="flex items-start justify-between gap-3">
            <div className="flex items-start gap-2">
              <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" />
              <div>
                <div className="font-medium">Live updates disconnected</div>
                <div className="mt-1 text-xs">
                  {failedSubscriptionLabels.join(', ')} stopped updating. Data may be stale until reconnect.
                </div>
              </div>
            </div>
            <div className="flex shrink-0 gap-2">
              <Button size="sm" variant="outline" onClick={retryLiveUpdates}>
                Reconnect
              </Button>
              <Button size="sm" variant="outline" onClick={refreshActivityData}>
                Refresh now
              </Button>
            </div>
          </div>
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
                    <span className="text-muted-foreground">
                      Value: {formatZatoshisToZec(tx.value)} ZEC
                      {fiatSettings?.enabled && exchangeRate && (
                        <span className={fiatIsStale ? "text-muted-foreground/60" : undefined} title={fiatIsStale ? "Exchange rate may be outdated" : undefined}>
                          {' '}({formatFiat(zatoshisToFiat(tx.value, exchangeRate.price), exchangeRate.currency)})
                        </span>
                      )}
                    </span>
                    <span className="text-muted-foreground">Fee: {formatZatoshisToZec(tx.fee)} ZEC</span>
                  </div>
                  <MemoDisplay
                    tx={tx}
                    isExpanded={expandedMemos.has(tx.txid)}
                    onToggleExpanded={() => toggleMemoExpanded(tx.txid)}
                    copiedMemo={copiedMemo}
                    copyError={copyError}
                    onCopyMemo={(txid, content) => void copyMemo(txid, content)}
                  />
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
