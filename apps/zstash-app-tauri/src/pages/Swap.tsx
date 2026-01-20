import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { ArrowLeftRight, AlertCircle, ArrowRight, Check } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { DEFAULT_NON_ZEC_ASSET_ID, getToZecTokens, ZEC_ASSET_ID } from '../data/supportedTokens';
import { getReceiveAddress, requestSwapQuote } from '../services/ipc';
import { parseSwapError } from '../utils/swap';

export type SwapQuoteLocationState = {
  quoteId: string;
  quote: IPC.SwapQuote;
};

export function Swap(props: { wallet: IPC.WalletInfo; activeAccountId: number | null }) {
  const { wallet, activeAccountId } = props;
  const navigate = useNavigate();

  const [inputAsset, setInputAsset] = useState(DEFAULT_NON_ZEC_ASSET_ID);
  const [inputAmount, setInputAmount] = useState('');
  const [destinationAddress, setDestinationAddress] = useState<string>('');
  const [refundAddress, setRefundAddress] = useState('');
  const [loadingAddress, setLoadingAddress] = useState(false);

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Swap-to-ZEC always targets ZEC in v1.
  const outputAsset = ZEC_ASSET_ID;

  const canSubmit = useMemo(() => {
    if (wallet.network !== 'Mainnet') return false;
    if (activeAccountId == null) return false;
    if (!inputAsset.trim()) return false;
    if (!inputAmount.trim()) return false;
    if (!destinationAddress.trim()) return false;
    if (!refundAddress.trim()) return false;
    return true;
  }, [wallet.network, activeAccountId, inputAsset, inputAmount, destinationAddress, refundAddress]);

  useEffect(() => {
    let cancelled = false;

    async function loadDefaultAddress() {
      if (wallet.network !== 'Mainnet') return;
      if (activeAccountId == null) return;

      setLoadingAddress(true);
      try {
        const res = await getReceiveAddress({
          account_id: activeAccountId,
          address_type: 'ShieldedOnly',
        });
        if (cancelled) return;

        if ('err' in res) {
          setError(res.err.message);
          return;
        }

        setDestinationAddress(res.ok.address.encoded);
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : 'Failed to load destination address');
      } finally {
        if (!cancelled) setLoadingAddress(false);
      }
    }

    loadDefaultAddress();
    return () => {
      cancelled = true;
    };
  }, [wallet.network, activeAccountId]);

  // Clear error when form inputs change
  useEffect(() => {
    setError(null);
  }, [inputAsset, inputAmount, destinationAddress, refundAddress]);

  if (wallet.network !== 'Mainnet') {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <ArrowLeftRight className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Swap</h1>
        </div>
        <Card>
          <CardContent className="pt-6">
            <div className="flex items-start gap-3">
              <AlertCircle className="h-5 w-5 text-muted-foreground shrink-0 mt-0.5" />
              <p className="text-muted-foreground">
                Swaps are only supported for Mainnet wallets in v1.
              </p>
            </div>
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
        <h1 className="text-2xl font-bold">Swap</h1>
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        <Card className="h-full border-primary bg-primary/5">
          <CardHeader>
            <CardTitle className="text-lg flex items-center justify-between gap-2">
              <span>Swap to ZEC</span>
              <span className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
                <Check className="h-3 w-3" />
                Current
              </span>
            </CardTitle>
            <CardDescription>Convert other assets into ZEC</CardDescription>
          </CardHeader>
        </Card>
        <Link to="/swap/from-zec" className="block">
          <Card className="h-full hover:border-primary/50 transition-colors">
            <CardHeader>
              <CardTitle className="text-lg flex items-center gap-2">
                Swap from ZEC
                <ArrowRight className="h-4 w-4" />
              </CardTitle>
              <CardDescription>Convert ZEC into other assets</CardDescription>
            </CardHeader>
          </Card>
        </Link>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Swap to ZEC</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="inputAsset">Input asset</Label>
            <select
              id="inputAsset"
              value={inputAsset}
              onChange={(e) => setInputAsset(e.currentTarget.value)}
              disabled={submitting}
              className="flex h-9 w-full rounded-none border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {getToZecTokens().map((t) => (
                <option key={t.id} value={t.id}>
                  {t.label}
                </option>
              ))}
            </select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="inputAmount">Amount (input asset units)</Label>
            <Input
              id="inputAmount"
              value={inputAmount}
              onChange={(e) => setInputAmount(e.currentTarget.value)}
              inputMode="decimal"
              placeholder="0.0"
              disabled={submitting}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="destinationAddress">Destination ZEC address</Label>
            <Input
              id="destinationAddress"
              value={destinationAddress}
              onChange={(e) => setDestinationAddress(e.currentTarget.value)}
              placeholder="u1... / zs... / etc"
              disabled={loadingAddress || submitting}
              className="font-mono"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="refundAddress">Refund address (origin chain)</Label>
            <Input
              id="refundAddress"
              value={refundAddress}
              onChange={(e) => setRefundAddress(e.currentTarget.value)}
              placeholder="Your address on the input asset chain for refunds if the swap fails"
              className="font-mono"
              disabled={submitting}
            />
          </div>

          {error && (
            <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}

          <Button
            disabled={!canSubmit || submitting}
            onClick={async () => {
              if (!canSubmit) return;
              setSubmitting(true);
              setError(null);

              try {
                const res = await requestSwapQuote({
                  swap_type: 'ToZec',
                  input_asset: inputAsset,
                  input_amount: inputAmount,
                  output_asset: outputAsset,
                  destination_address: destinationAddress.trim() ? destinationAddress.trim() : null,
                  refund_address: refundAddress.trim() ? refundAddress.trim() : null,
                });

                if ('err' in res) {
                  setError(parseSwapError(res.err.message));
                  return;
                }

                navigate('/swap/quote', {
                  state: { quoteId: res.ok.quote_id, quote: res.ok.quote } satisfies SwapQuoteLocationState,
                });
              } catch (e) {
                setError(e instanceof Error ? e.message : 'Failed to request quote');
              } finally {
                setSubmitting(false);
              }
            }}
            className="w-full"
          >
            {submitting ? 'Requesting quote...' : 'Get quote'}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
