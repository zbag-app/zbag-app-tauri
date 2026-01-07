import { QRCodeSVG } from 'qrcode.react';
import { useEffect, useMemo, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';
import { getSwapStatus } from '../services/ipc';
import { onSwapChanged } from '../services/events';
import type { SwapDepositLocationState } from './SwapQuote';

function formatDeadline(deadlineMs: number): string {
  const ms = deadlineMs - Date.now();
  const secs = Math.max(0, Math.floor(ms / 1000));
  const mins = Math.floor(secs / 60);
  const rem = secs % 60;
  return `${mins}:${rem.toString().padStart(2, '0')}`;
}

export function SwapDeposit() {
  const navigate = useNavigate();
  const location = useLocation();
  const state = location.state as SwapDepositLocationState | null;
  const [swap, setSwap] = useState<IPC.SwapInfo | null>(state?.swap ?? null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (location.state != null) {
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.pathname, location.state, navigate]);

  const expired = useMemo(() => {
    if (!swap?.deadline) return false;
    return Date.now() >= swap.deadline;
  }, [swap?.deadline]);

  useEffect(() => {
    if (!swap) return;
    const swapId = swap.id;
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    async function run() {
      unlisten = await onSwapChanged((event) => {
        if (cancelled) return;
        if (event.swap.id !== swapId) return;
        setSwap(event.swap);
      });
    }

    run();
    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [swap?.id]);

  if (!swap) {
    return (
      <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 760 }}>
        <h1>Swap Deposit</h1>
        <div>
          Missing swap. Return to <Link to="/swap">Swap</Link>.
        </div>
      </div>
    );
  }

  const depositText = swap.deposit_memo
    ? `${swap.deposit_address}\nMemo: ${swap.deposit_memo}`
    : swap.deposit_address ?? '';

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 760 }}>
      <h1>Deposit</h1>

      <div style={{ display: 'grid', gap: 6 }}>
        <div style={{ fontWeight: 600 }}>Status: {swap.state}</div>
        {swap.deadline ? (
          <div style={{ fontSize: 13, opacity: 0.8 }}>
            Deadline: {expired ? 'Expired' : `in ${formatDeadline(swap.deadline)}`}
          </div>
        ) : null}
      </div>

      {swap.deposit_address ? (
        <div style={{ display: 'grid', gap: 10 }}>
          <div style={{ display: 'grid', gap: 6 }}>
            <div style={{ fontSize: 13, opacity: 0.8 }}>Deposit address</div>
            <code style={{ wordBreak: 'break-all' }}>{swap.deposit_address}</code>
          </div>
          {swap.deposit_memo ? (
            <div style={{ display: 'grid', gap: 6 }}>
              <div style={{ fontSize: 13, opacity: 0.8 }}>Deposit memo/tag</div>
              <code style={{ wordBreak: 'break-all' }}>{swap.deposit_memo}</code>
            </div>
          ) : null}
          <QRCodeSVG value={depositText} size={240} />
        </div>
      ) : (
        <div>No deposit address available.</div>
      )}

      {swap.last_error ? <div style={{ color: '#b91c1c' }}>Last error: {swap.last_error}</div> : null}
      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
        <button
          type="button"
          onClick={async () => {
            setError(null);
            const res = await getSwapStatus({ swap_id: swap.id });
            if ('err' in res) {
              setError(res.err.message);
              return;
            }
            setSwap(res.ok.swap);
          }}
        >
          Refresh status
        </button>
        <Link to="/activity">Activity</Link>
        <Link to="/swap">New swap</Link>
      </div>
    </div>
  );
}
