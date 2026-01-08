import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { ArrowLeftRight, AlertCircle } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { supportedTokens } from '../data/supportedTokens';
import { getReceiveAddress, requestSwapQuote } from '../services/ipc';

export type SwapQuoteLocationState = {
  quoteId: string;
  quote: IPC.SwapQuote;
};

export function Swap(props: { wallet: IPC.WalletInfo; activeAccountId: number | null }) {
  const { wallet, activeAccountId } = props;
  const navigate = useNavigate();

  const [swapType, setSwapType] = useState<IPC.SwapType>('ToZec');
  const [inputAsset, setInputAsset] = useState('near:mainnet:native');
  const [inputAmount, setInputAmount] = useState('');
  const [destinationAddress, setDestinationAddress] = useState<string>('');
  const [loadingAddress, setLoadingAddress] = useState(false);

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const outputAsset = 'zcash:mainnet:native';

  const canSubmit = useMemo(() => {
    if (wallet.network !== 'Mainnet') return false;
    if (swapType !== 'ToZec') return false;
    if (activeAccountId == null) return false;
    if (!inputAsset.trim()) return false;
    if (!inputAmount.trim()) return false;
    if (!destinationAddress.trim()) return false;
    return true;
  }, [wallet.network, swapType, activeAccountId, inputAsset, inputAmount, destinationAddress]);

  useEffect(() => {
    let cancelled = false;

    async function loadDefaultAddress() {
      if (wallet.network !== 'Mainnet') return;
      if (activeAccountId == null) return;

      setLoadingAddress(true);
      const res = await getReceiveAddress({
        account_id: activeAccountId,
        address_type: 'ShieldedOnly',
      });
      if (cancelled) return;
      setLoadingAddress(false);

      if ('err' in res) {
        setError(res.err.message);
        return;
      }

      setDestinationAddress(res.ok.address.encoded);
    }

    loadDefaultAddress();
    return () => {
      cancelled = true;
    };
  }, [wallet.network, activeAccountId]);

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

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Swap Details</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="swapType">Swap type</Label>
            <select
              id="swapType"
              value={swapType}
              onChange={(e) => setSwapType(e.currentTarget.value as IPC.SwapType)}
              className="flex h-9 w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <option value="ToZec">To ZEC</option>
              <option value="FromZec">From ZEC</option>
            </select>
          </div>

          {swapType === 'FromZec' && (
            <div className="rounded-lg border border-border bg-muted/50 p-3 text-sm text-muted-foreground">
              Swap-from-ZEC is not implemented yet.{' '}
              <Link to="/swap/from-zec" className="text-primary hover:underline">
                Open page
              </Link>
            </div>
          )}

          {swapType === 'ToZec' && (
            <>
              <div className="space-y-2">
                <Label htmlFor="inputAsset">Input asset</Label>
                <select
                  id="inputAsset"
                  value={inputAsset}
                  onChange={(e) => setInputAsset(e.currentTarget.value)}
                  className="flex h-9 w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                  {supportedTokens
                    .filter((t) => t.id !== outputAsset)
                    .map((t) => (
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
                  placeholder="0.0"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="destinationAddress">Destination ZEC address</Label>
                <textarea
                  id="destinationAddress"
                  rows={2}
                  value={destinationAddress}
                  onChange={(e) => setDestinationAddress(e.currentTarget.value)}
                  placeholder="u1... / zs... / etc"
                  disabled={loadingAddress}
                  className="flex w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 font-mono"
                />
              </div>

              {error && (
                <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                  {error}
                </div>
              )}

              <Button
                disabled={!canSubmit || submitting}
                onClick={async () => {
                  if (!canSubmit) return;
                  setSubmitting(true);
                  setError(null);

                  const res = await requestSwapQuote({
                    swap_type: swapType,
                    input_asset: inputAsset,
                    input_amount: inputAmount,
                    output_asset: outputAsset,
                    destination_address: destinationAddress.trim() ? destinationAddress.trim() : null,
                    refund_address: null,
                  });
                  setSubmitting(false);

                  if ('err' in res) {
                    setError(res.err.message);
                    return;
                  }

                  navigate('/swap/quote', {
                    state: { quoteId: res.ok.quote_id, quote: res.ok.quote } satisfies SwapQuoteLocationState,
                  });
                }}
                className="w-full"
              >
                {submitting ? 'Requesting quote...' : 'Get quote'}
              </Button>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
