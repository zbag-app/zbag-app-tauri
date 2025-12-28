import { useMemo, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';
import { startSwap } from '../services/ipc';
import type { SwapQuoteLocationState } from './Swap';

function formatDeadline(deadlineMs: number): string {
  const ms = deadlineMs - Date.now();
  const secs = Math.max(0, Math.floor(ms / 1000));
  const mins = Math.floor(secs / 60);
  const rem = secs % 60;
  return `${mins}:${rem.toString().padStart(2, '0')}`;
}

export type SwapDepositLocationState = {
  swap: IPC.SwapInfo;
};

export function SwapQuote() {
  const navigate = useNavigate();
  const location = useLocation();
  const state = location.state as SwapQuoteLocationState | null;
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const quoteId = state?.quoteId ?? null;
  const quote = state?.quote ?? null;

  const expired = useMemo(() => {
    if (!quote) return false;
    return Date.now() >= quote.deadline;
  }, [quote]);

  if (!quoteId || !quote) {
    return (
      <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 760 }}>
        <h1>Swap Quote</h1>
        <div>
          Missing quote. Return to <Link to="/swap">Swap</Link>.
        </div>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 760 }}>
      <h1>Quote</h1>

      <div style={{ display: 'grid', gap: 6 }}>
        <div>
          {quote.input_amount} {quote.input_asset} → {quote.output_amount} {quote.output_asset}
        </div>
        <div style={{ fontSize: 13, opacity: 0.8 }}>
          Fee: {quote.fee_amount} {quote.fee_asset}
        </div>
        <div style={{ fontSize: 13, opacity: 0.8 }}>Rate: {quote.rate}</div>
        <div style={{ fontSize: 13, opacity: 0.8 }}>
          Expires in: {formatDeadline(quote.deadline)}
        </div>
      </div>

      {expired ? <div style={{ color: '#b91c1c' }}>Quote expired.</div> : null}
      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'flex', gap: 12 }}>
        <button
          type="button"
          disabled={expired || submitting}
          onClick={async () => {
            setSubmitting(true);
            setError(null);
            const res = await startSwap({
              quote_id: quoteId,
              allow_transparent_interaction: false,
              reauth_token: null,
            });
            setSubmitting(false);

            if ('err' in res) {
              setError(res.err.message);
              return;
            }

            navigate('/swap/deposit', { state: { swap: res.ok.swap } satisfies SwapDepositLocationState });
          }}
        >
          {submitting ? 'Starting…' : 'Start swap'}
        </button>
        <Link to="/swap">Back</Link>
      </div>
    </div>
  );
}

