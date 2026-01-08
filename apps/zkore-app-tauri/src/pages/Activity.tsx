import { useCallback, useEffect, useMemo, useState } from 'react';
import type * as IPC from '../types/ipc';
import { listSwaps, listTransactions, reauthWallet, retryBroadcast } from '../services/ipc';
import { onSwapChanged, onTransactionChanged } from '../services/events';
import { formatZatoshisToZec } from '../utils/zec';

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

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 920 }}>
      <h1>Activity</h1>

      {activeAccountId == null ? <div>No active account.</div> : null}
      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
        <button
          type="button"
          onClick={() => {
            void load(offset);
            void loadSwaps();
          }}
          disabled={loading || activeAccountId == null}
        >
          Refresh
        </button>
        <div style={{ fontSize: 12, opacity: 0.7 }}>
          Showing {transactions.length} of {totalCount}
        </div>
      </div>

      <div style={{ display: 'grid', gap: 8 }}>
        <h2 style={{ margin: '8px 0 0' }}>Swaps</h2>
        {swaps.length === 0 ? <div style={{ fontSize: 13, opacity: 0.7 }}>No swaps.</div> : null}
        {swaps.map((s) => {
          const expired = s.deadline != null && Date.now() >= s.deadline;
          return (
            <div key={s.id} style={{ border: '1px solid rgba(0,0,0,0.12)', borderRadius: 8, padding: 12 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
                <div style={{ display: 'grid', gap: 4 }}>
                  <div style={{ fontWeight: 600 }}>
                    {s.swap_type}: {s.input_amount} {s.input_asset} → {s.output_amount ?? '?'} {s.output_asset}
                  </div>
                  {s.deposit_address ? (
                    <div style={{ fontSize: 12, opacity: 0.8 }}>
                      Deposit: <code style={{ wordBreak: 'break-all' }}>{s.deposit_address}</code>
                    </div>
                  ) : null}
                  {s.deposit_memo ? (
                    <div style={{ fontSize: 12, opacity: 0.8 }}>
                      Memo: <code style={{ wordBreak: 'break-all' }}>{s.deposit_memo}</code>
                    </div>
                  ) : null}
                  {s.deadline ? (
                    <div style={{ fontSize: 12, opacity: 0.8 }}>
                      Deadline: {expired ? <span style={{ color: '#b91c1c' }}>Expired</span> : new Date(s.deadline).toLocaleString()}
                    </div>
                  ) : null}
                  {s.last_error ? (
                    <div style={{ fontSize: 12, color: '#b91c1c' }}>Last error: {s.last_error}</div>
                  ) : null}
                </div>
                <div style={{ textAlign: 'right' }}>
                  <div style={{ fontWeight: 600 }}>{s.state}</div>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      <div style={{ display: 'grid', gap: 8 }}>
        {loading && transactions.length === 0 ? <div>Loading…</div> : null}
        {!loading && transactions.length === 0 && activeAccountId != null ? <div>No transactions.</div> : null}

        {transactions.map((tx) => {
          return (
            <div key={tx.txid} style={{ border: '1px solid rgba(0,0,0,0.12)', borderRadius: 8, padding: 12 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
                <div style={{ display: 'grid', gap: 4 }}>
                  <code style={{ wordBreak: 'break-all', fontSize: 12 }}>{tx.txid}</code>
                  <div style={{ display: 'flex', gap: 12, fontSize: 14 }}>
                    <span>{tx.tx_type}</span>
                    <span style={{ opacity: 0.8 }}>Value: {formatZatoshisToZec(tx.value)} ZEC</span>
                    <span style={{ opacity: 0.8 }}>Fee: {formatZatoshisToZec(tx.fee)} ZEC</span>
                    <span style={{ opacity: 0.8 }}>Memo: {tx.memo_present ? 'Yes' : 'No'}</span>
                  </div>
                </div>
                <div style={{ textAlign: 'right' }}>
                  <div style={{ fontWeight: 600 }}>{tx.status}</div>
                </div>
              </div>

              {tx.can_retry_broadcast ? (
                <div style={{ marginTop: 10, display: 'grid', gap: 8 }}>
                  {tx.last_error ? (
                    <div style={{ fontSize: 13, color: '#b91c1c' }}>
                      Last error: {tx.last_error}
                    </div>
                  ) : null}

                  {retryTxid === tx.txid ? (
                    <div style={{ display: 'grid', gap: 8, maxWidth: 520 }}>
                      <label style={{ display: 'grid', gap: 4 }}>
                        <span>Password</span>
                        <input
                          type="password"
                          value={retryPassword}
                          onChange={(e) => setRetryPassword(e.currentTarget.value)}
                          disabled={retrying}
                        />
                      </label>
                      {retryError ? <div style={{ color: 'crimson' }}>{retryError}</div> : null}
                      <div style={{ display: 'flex', gap: 12 }}>
                        <button type="button" onClick={submitRetry} disabled={!retryPassword || retrying}>
                          {retrying ? 'Retrying…' : 'Retry broadcast'}
                        </button>
                        <button
                          type="button"
                          onClick={() => {
                            setRetryTxid(null);
                            setRetryPassword('');
                            setRetryError(null);
                          }}
                          disabled={retrying}
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  ) : (
                    <button
                      type="button"
                      onClick={() => {
                        setRetryTxid(tx.txid);
                        setRetryPassword('');
                        setRetryError(null);
                      }}
                    >
                      Retry broadcast
                    </button>
                  )}
                </div>
              ) : null}
            </div>
          );
        })}
      </div>

      <div style={{ display: 'flex', gap: 12 }}>
        <button
          type="button"
          disabled={!canPagePrev || loading}
          onClick={() => void load(Math.max(0, offset - limit))}
        >
          Prev
        </button>
        <button
          type="button"
          disabled={!canPageNext || loading}
          onClick={() => void load(offset + limit)}
        >
          Next
        </button>
      </div>
    </div>
  );
}
