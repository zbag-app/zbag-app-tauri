import { useEffect, useMemo, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { ArrowLeftRight, Clock, ArrowLeft } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
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
  const [state] = useState<SwapQuoteLocationState | null>(() => location.state as SwapQuoteLocationState | null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const quoteId = state?.quoteId ?? null;
  const quote = state?.quote ?? null;

  useEffect(() => {
    if (location.state != null) {
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.pathname, location.state, navigate]);

  const expired = useMemo(() => {
    if (!quote) return false;
    return Date.now() >= quote.deadline;
  }, [quote]);

  if (!quoteId || !quote) {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <ArrowLeftRight className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Swap Quote</h1>
        </div>

        <Card>
          <CardContent className="pt-6">
            <p className="text-muted-foreground">
              Missing quote. Return to <Link to="/swap" className="text-primary hover:underline">Swap</Link>.
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <ArrowLeftRight className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Quote</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Swap Details</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="rounded-lg bg-muted/50 p-4">
            <div className="text-lg font-semibold">
              {quote.input_amount} {quote.input_asset} → {quote.output_amount} {quote.output_asset}
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4 text-sm">
            <div className="space-y-1">
              <span className="text-muted-foreground">Fee</span>
              <div className="font-semibold">
                {quote.fee_amount} {quote.fee_asset}
              </div>
            </div>
            <div className="space-y-1">
              <span className="text-muted-foreground">Rate</span>
              <div className="font-semibold">{quote.rate}</div>
            </div>
            <div className="space-y-1 col-span-2">
              <span className="text-muted-foreground flex items-center gap-1">
                <Clock className="h-3 w-3" />
                Expires in
              </span>
              <div className={`font-mono font-semibold ${expired ? 'text-destructive' : ''}`}>
                {expired ? 'Expired' : formatDeadline(quote.deadline)}
              </div>
            </div>
          </div>

          {expired && (
            <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              Quote expired.
            </div>
          )}

          {error && (
            <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}

          <div className="flex gap-3">
            <Button
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
              className="flex-1"
            >
              {submitting ? 'Starting...' : 'Start swap'}
            </Button>
            <Link to="/swap">
              <Button variant="outline">
                <ArrowLeft className="h-4 w-4" />
                Back
              </Button>
            </Link>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
