import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { ArrowLeftRight, ArrowLeft } from 'lucide-react';
import * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { PrivacyWarning } from '../components/swap/PrivacyWarning';
import { getFromZecTokens, getTokenById, ZEC_ASSET_ID, DEFAULT_NON_ZEC_ASSET_ID } from '../data/supportedTokens';
import { getReceiveAddress, reauthWallet, requestSwapQuote, startSwap } from '../services/ipc';
import { parseZecToZatoshis } from '../utils/zec';
import { parseSwapError } from '../utils/swap';
import { formatAtomicAmount } from '../utils/amounts';

export function SwapFromZec(props: { wallet: IPC.WalletInfo; activeAccountId: number | null }) {
  const { wallet, activeAccountId } = props;
  const navigate = useNavigate();

  const [outputAsset, setOutputAsset] = useState(DEFAULT_NON_ZEC_ASSET_ID);
  const [inputAmountZec, setInputAmountZec] = useState('');
  const [destinationAddress, setDestinationAddress] = useState('');
  const [refundAddress, setRefundAddress] = useState('');
  const [loadingRefundAddress, setLoadingRefundAddress] = useState(false);

  const [quoteId, setQuoteId] = useState<string | null>(null);
  const [quote, setQuote] = useState<IPC.SwapQuote | null>(null);

  const [password, setPassword] = useState('');
  const [reauthToken, setReauthToken] = useState<string | null>(null);
  // FromZec swaps always require transparent interaction (deposit address is transparent)
  const [privacyAck, setPrivacyAck] = useState(false);

  const [submittingQuote, setSubmittingQuote] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canQuote = useMemo(() => {
    if (wallet.network !== 'Mainnet') return false;
    if (activeAccountId == null) return false;
    if (!outputAsset.trim()) return false;
    if (!inputAmountZec.trim()) return false;
    if (!destinationAddress.trim()) return false;
    if (!refundAddress.trim()) return false;
    return true;
  }, [wallet.network, activeAccountId, outputAsset, inputAmountZec, destinationAddress, refundAddress]);

  // Auto-populate refund address from wallet's shielded address
  useEffect(() => {
    let cancelled = false;

    async function loadRefundAddress() {
      if (wallet.network !== 'Mainnet') return;
      if (activeAccountId == null) return;

      setLoadingRefundAddress(true);
      const res = await getReceiveAddress({
        account_id: activeAccountId,
        address_type: 'ShieldedOnly',
      });
      if (cancelled) return;
      setLoadingRefundAddress(false);

      if ('err' in res) {
        setError(res.err.message);
        return;
      }

      setRefundAddress(res.ok.address.encoded);
    }

    loadRefundAddress();
    return () => {
      cancelled = true;
    };
  }, [wallet.network, activeAccountId]);

  // Clear error when form inputs change
  useEffect(() => {
    setError(null);
  }, [outputAsset, inputAmountZec, destinationAddress, refundAddress]);

  // Format min output amount with token symbol
  const formattedMinOutput = useMemo(() => {
    if (!quote) return null;
    const token = getTokenById(quote.output_asset);
    if (!token) return quote.min_output_amount;
    const formatted = formatAtomicAmount(quote.min_output_amount, token.decimals);
    return `${formatted} ${token.label}`;
  }, [quote]);

  if (wallet.network !== 'Mainnet') {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <ArrowLeftRight className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Swap From ZEC</h1>
        </div>

        <Card>
          <CardContent className="pt-6">
            <p className="text-muted-foreground">
              Swaps are only supported for Mainnet wallets in v1.
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
        <h1 className="text-2xl font-bold">Swap From ZEC</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Swap Details</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="outputAsset">Target asset</Label>
            <select
              id="outputAsset"
              value={outputAsset}
              onChange={(e) => setOutputAsset(e.currentTarget.value)}
              className="flex h-9 w-full rounded-none border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {getFromZecTokens().map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.label}
                  </option>
                ))}
            </select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="inputAmount">Amount (ZEC)</Label>
            <Input
              id="inputAmount"
              value={inputAmountZec}
              onChange={(e) => setInputAmountZec(e.currentTarget.value)}
              placeholder="0.0"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="destinationAddress">Destination address (target asset chain)</Label>
            <Input
              id="destinationAddress"
              value={destinationAddress}
              onChange={(e) => setDestinationAddress(e.currentTarget.value)}
              placeholder="Paste the destination address for the target asset"
              className="font-mono"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="refundAddress">Refund address (ZEC)</Label>
            <Input
              id="refundAddress"
              value={refundAddress}
              onChange={(e) => setRefundAddress(e.currentTarget.value)}
              placeholder="Your ZEC address for refunds"
              disabled={loadingRefundAddress}
              className="font-mono"
            />
          </div>

          <PrivacyWarning acknowledged={privacyAck} onAcknowledgedChange={setPrivacyAck} />

          <Button
            disabled={!canQuote || submittingQuote}
            onClick={async () => {
              if (!canQuote) return;
              setSubmittingQuote(true);
              setError(null);
              setQuote(null);
              setQuoteId(null);
              setReauthToken(null);

              const parseResult = parseZecToZatoshis(inputAmountZec);
              if ('err' in parseResult) {
                setError(parseResult.err);
                setSubmittingQuote(false);
                return;
              }
              const zatoshis = parseResult.ok;

              const res = await requestSwapQuote({
                swap_type: 'FromZec',
                input_asset: ZEC_ASSET_ID,
                input_amount: zatoshis,
                output_asset: outputAsset,
                destination_address: destinationAddress.trim() ? destinationAddress.trim() : null,
                refund_address: refundAddress.trim() ? refundAddress.trim() : null,
              });
              setSubmittingQuote(false);

              if ('err' in res) {
                setError(parseSwapError(res.err.message));
                return;
              }

              setQuoteId(res.ok.quote_id);
              setQuote(res.ok.quote);
            }}
            className="w-full"
          >
            {submittingQuote ? 'Requesting quote...' : 'Get quote'}
          </Button>
        </CardContent>
      </Card>

      {quote && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Quote</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="rounded-none bg-muted/50 p-4">
              <div className="text-lg font-semibold">
                {quote.input_amount_formatted} → {quote.output_amount_formatted}
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4 text-sm">
              <div className="space-y-1">
                <span className="text-muted-foreground">Min. output</span>
                <div className="font-semibold">{formattedMinOutput}</div>
              </div>
              <div className="space-y-1">
                <span className="text-muted-foreground">Est. time</span>
                <div className="font-semibold">{Math.ceil(quote.time_estimate_secs / 60)} min</div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {quoteId && quote && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Confirm Swap</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="password">Password</Label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.currentTarget.value)}
                disabled={starting}
                placeholder="Enter your password"
              />
            </div>

            <div className="flex gap-3">
              <Button
                disabled={!password.trim() || starting || !privacyAck}
                onClick={async () => {
                  if (!quoteId) return;
                  if (!privacyAck) {
                    setError('This swap requires transparent interaction. Confirm the privacy acknowledgement to continue.');
                    return;
                  }

                  setStarting(true);
                  setError(null);

                  const token =
                    reauthToken ??
                    (await (async () => {
                      const res = await reauthWallet({
                        wallet_id: wallet.id,
                        password,
                        purpose: 'Spend',
                      });
                      if ('err' in res) throw new Error(res.err.message);
                      return res.ok.reauth_token;
                    })()).toString();

                  try {
                    if (!reauthToken) {
                      setReauthToken(token);
                    }

                    const startRes = await startSwap({
                      quote_id: quoteId,
                      allow_transparent_interaction: privacyAck,
                      reauth_token: token,
                    });

                    if ('err' in startRes) {
                      if (startRes.err.code === IPC.ErrorCodes.PRIVACY_ACK_REQUIRED) {
                        setError(
                          'This swap requires transparent interaction. Confirm the privacy acknowledgement to continue.'
                        );
                        setStarting(false);
                        return;
                      }
                      setError(parseSwapError(startRes.err.message));
                      setStarting(false);
                      return;
                    }

                    setPassword('');
                    setReauthToken(null);
                    navigate('/activity');
                  } catch (e) {
                    setError(e instanceof Error ? e.message : 'Failed to start swap');
                    setStarting(false);
                  }
                }}
                className="flex-1"
              >
                {starting ? 'Starting...' : 'Start swap'}
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
      )}

      {error && (
        <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}
    </div>
  );
}
