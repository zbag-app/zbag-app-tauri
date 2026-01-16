import { QRCodeSVG } from 'qrcode.react';
import { useEffect, useMemo, useState } from 'react';
import { Link, useLocation, useNavigate, useParams } from 'react-router-dom';
import { ArrowLeftRight, RefreshCw, Copy, Clock } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Badge } from '../components/ui/badge';
import { useNowMs } from '../hooks/useNowMs';
import { formatCountdown } from '../lib/time';
import { getSwapStatus, refreshSwapStatus } from '../services/ipc';
import { onSwapChanged } from '../services/events';
import type { SwapDepositLocationState } from './SwapQuote';

export function SwapDeposit() {
  const navigate = useNavigate();
  const location = useLocation();
  const { swapId } = useParams<{ swapId: string }>();
  const state = location.state as SwapDepositLocationState | null;
  const [swap, setSwap] = useState<IPC.SwapInfo | null>(state?.swap ?? null);
  const [error, setError] = useState<string | null>(null);
  const nowMs = useNowMs(Boolean(swap?.deadline));
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  // Clear location state after reading it
  useEffect(() => {
    if (location.state != null) {
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.pathname, location.state, navigate]);

  // Load swap from backend if we have a swapId but no swap from state
  useEffect(() => {
    if (swap != null || swapId == null) return;

    let cancelled = false;
    setLoading(true);

    const id = swapId; // Capture for closure
    async function loadSwap() {
      const res = await getSwapStatus({ swap_id: id });
      if (cancelled) return;
      setLoading(false);
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setSwap(res.ok.swap);
    }

    loadSwap();
    return () => {
      cancelled = true;
    };
  }, [swap, swapId]);

  const expired = useMemo(() => {
    if (!swap?.deadline) return false;
    return nowMs >= swap.deadline;
  }, [swap?.deadline, nowMs]);

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

  const copyAddress = async () => {
    if (swap?.deposit_address) {
      await navigator.clipboard.writeText(swap.deposit_address);
    }
  };

  if (loading) {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <ArrowLeftRight className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Swap Deposit</h1>
        </div>

        <Card>
          <CardContent className="pt-6">
            <p className="text-muted-foreground animate-pulse">Loading swap...</p>
          </CardContent>
        </Card>
      </div>
    );
  }

  if (!swap) {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <ArrowLeftRight className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Swap Deposit</h1>
        </div>

        <Card>
          <CardContent className="pt-6 space-y-4">
            {error && (
              <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                {error}
              </div>
            )}
            <p className="text-muted-foreground">
              Missing swap. Return to <Link to="/swap" className="text-primary hover:underline">Swap</Link>.
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  const depositText = swap.deposit_memo
    ? `${swap.deposit_address}\nMemo: ${swap.deposit_memo}`
    : swap.deposit_address ?? '';

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <ArrowLeftRight className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Deposit</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            Status
            <Badge variant={swap.state === 'Completed' ? 'success' : 'secondary'}>
              {swap.state}
            </Badge>
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {swap.deadline && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Clock className="h-4 w-4" />
              Deadline: {expired ? 'Expired' : `in ${formatCountdown(swap.deadline, nowMs)}`}
            </div>
          )}

          {swap.deposit_address ? (
            <div className="space-y-4">
              <div className="space-y-2">
                <span className="text-sm text-muted-foreground">Deposit address</span>
                <code className="block text-sm break-all bg-muted px-3 py-2 rounded-none font-mono">
                  {swap.deposit_address}
                </code>
              </div>

              {swap.deposit_memo && (
                <div className="space-y-2">
                  <span className="text-sm text-muted-foreground">Deposit memo/tag</span>
                  <code className="block text-sm break-all bg-muted px-3 py-2 rounded-none font-mono">
                    {swap.deposit_memo}
                  </code>
                </div>
              )}

              <div className="flex justify-center p-4 bg-white rounded-none">
                <QRCodeSVG value={depositText} size={200} />
              </div>

              <Button variant="outline" onClick={copyAddress} className="w-full">
                <Copy className="h-4 w-4" />
                Copy address
              </Button>
            </div>
          ) : (
            <p className="text-muted-foreground">No deposit address available.</p>
          )}

          {swap.last_error && (
            <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              Last error: {swap.last_error}
            </div>
          )}

          {error && (
            <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}

          <div className="flex gap-3 flex-wrap">
            <Button
              variant="outline"
              disabled={refreshing}
              onClick={async () => {
                setError(null);
                setRefreshing(true);
                const res = await refreshSwapStatus({ swap_id: swap.id });
                setRefreshing(false);
                if ('err' in res) {
                  setError(res.err.message);
                  return;
                }
                setSwap(res.ok.swap);
              }}
            >
              <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
              {refreshing ? 'Refreshing...' : 'Refresh status'}
            </Button>
            <Link to="/activity">
              <Button variant="outline">Activity</Button>
            </Link>
            <Link to="/swap">
              <Button variant="outline">New swap</Button>
            </Link>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
