import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { CreditCard, ArrowLeft, Info, Clock } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { PrivacyWarning } from '../components/swap/PrivacyWarning';
import { getFromZecTokens, ZEC_ASSET_ID, DEFAULT_NON_ZEC_ASSET_ID, supportedTokens } from '../data/supportedTokens';
import { useNowMs } from '../hooks/useNowMs';
import { formatCountdown } from '../lib/time';
import { getReceiveAddress, reauthWallet, requestSwapQuote, startSwap } from '../services/ipc';

/**
 * CrossPay page - Pay recipients in other currencies using ZEC.
 *
 * This uses EXACT_OUTPUT swap mode where the user specifies the desired
 * output amount and the system calculates the required ZEC input.
 */
export function CrossPay(props: { wallet: IPC.WalletInfo; activeAccountId: number | null }) {
  const { wallet, activeAccountId } = props;
  const navigate = useNavigate();

  const [outputAsset, setOutputAsset] = useState(DEFAULT_NON_ZEC_ASSET_ID);
  const [outputAmount, setOutputAmount] = useState('');
  const [destinationAddress, setDestinationAddress] = useState('');
  const [refundAddress, setRefundAddress] = useState('');
  const [loadingRefundAddress, setLoadingRefundAddress] = useState(false);

  const [quoteId, setQuoteId] = useState<string | null>(null);
  const [quote, setQuote] = useState<IPC.SwapQuote | null>(null);
  const nowMs = useNowMs(quote != null);

  const [password, setPassword] = useState('');
  const [reauthToken, setReauthToken] = useState<string | null>(null);
  const [privacyAckRequired, setPrivacyAckRequired] = useState(false);
  const [privacyAck, setPrivacyAck] = useState(false);

  const [submittingQuote, setSubmittingQuote] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Get the selected token info for display
  const selectedToken = useMemo(() => {
    return supportedTokens.find((t) => t.id === outputAsset);
  }, [outputAsset]);

  const outputAmountValidationError = useMemo(() => {
    const trimmed = outputAmount.trim();
    if (!trimmed) return null;
    if (!selectedToken) return 'Select a valid target asset';
    if (!/^[0-9]+(?:\.[0-9]*)?$/.test(trimmed)) return 'Enter a valid amount (e.g., 1.23)';

    const maxDecimals = selectedToken.decimals;
    const [, frac = ''] = trimmed.split('.');
    if (frac.length > maxDecimals) return `Too many decimal places (max ${maxDecimals})`;

    if (/^0+(?:\.0*)?$/.test(trimmed)) return 'Amount must be greater than zero';
    return null;
  }, [outputAmount, selectedToken]);

  const canQuote = useMemo(() => {
    if (wallet.network !== 'Mainnet') return false;
    if (activeAccountId == null) return false;
    if (!selectedToken) return false;
    if (!outputAsset.trim()) return false;
    if (!outputAmount.trim()) return false;
    if (outputAmountValidationError) return false;
    if (!destinationAddress.trim()) return false;
    if (!refundAddress.trim()) return false;
    return true;
  }, [
    wallet.network,
    activeAccountId,
    selectedToken,
    outputAsset,
    outputAmount,
    outputAmountValidationError,
    destinationAddress,
    refundAddress,
  ]);

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

  const quoteExpired = useMemo(() => {
    if (!quote) return false;
    return nowMs >= quote.deadline;
  }, [quote, nowMs]);

  if (wallet.network !== 'Mainnet') {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <CreditCard className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">CrossPay</h1>
        </div>

        <Card>
          <CardContent className="pt-6">
            <p className="text-muted-foreground">
              CrossPay is only supported for Mainnet wallets.
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
          <CreditCard className="h-5 w-5 text-primary" />
        </div>
        <div>
          <h1 className="text-2xl font-bold">CrossPay</h1>
          <p className="text-sm text-muted-foreground">Pay recipients in other currencies using your ZEC</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Payment Details</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Info banner */}
          <div className="flex items-start gap-3 rounded-lg border border-border bg-muted/50 p-3">
            <Info className="h-5 w-5 text-muted-foreground shrink-0 mt-0.5" />
            <div className="text-sm text-muted-foreground">
              <p>
                CrossPay lets you pay someone in their preferred currency while spending your ZEC.
                Specify how much they should receive, and we will calculate the ZEC amount needed.
              </p>
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="outputAsset">Currency to pay</Label>
            <select
              id="outputAsset"
              value={outputAsset}
              onChange={(e) => {
                setOutputAsset(e.currentTarget.value);
                setQuote(null);
                setQuoteId(null);
                setError(null);
              }}
              className="flex h-9 w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {getFromZecTokens().map((t) => (
                <option key={t.id} value={t.id}>
                  {t.label}
                </option>
              ))}
            </select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="outputAmount">Amount recipient will receive</Label>
            <div className="relative">
              <Input
                id="outputAmount"
                value={outputAmount}
                inputMode="decimal"
                onChange={(e) => {
                  setOutputAmount(e.currentTarget.value);
                  setQuote(null);
                  setQuoteId(null);
                  setError(null);
                }}
                placeholder="0.0"
                className="pr-16"
              />
              <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground">
                {selectedToken?.label ?? outputAsset}
              </span>
            </div>
            {outputAmountValidationError && <div className="text-sm text-destructive">{outputAmountValidationError}</div>}
          </div>

          <div className="space-y-2">
            <Label htmlFor="destinationAddress">Recipient address</Label>
            <textarea
              id="destinationAddress"
              rows={2}
              value={destinationAddress}
              onChange={(e) => {
                setDestinationAddress(e.currentTarget.value);
                setQuote(null);
                setQuoteId(null);
                setError(null);
              }}
              placeholder={`Paste the recipient's ${selectedToken?.blockchain?.toUpperCase() ?? 'destination'} address`}
              className="flex w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring font-mono"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="refundAddress">Refund address (ZEC)</Label>
            <textarea
              id="refundAddress"
              rows={2}
              value={refundAddress}
              onChange={(e) => {
                setRefundAddress(e.currentTarget.value);
                setQuote(null);
                setQuoteId(null);
                setError(null);
              }}
              placeholder="Your ZEC address for refunds if the payment fails"
              disabled={loadingRefundAddress}
              className="flex w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 font-mono"
            />
          </div>

          <Button
            disabled={!canQuote || submittingQuote}
            onClick={async () => {
              if (!canQuote) return;
              setSubmittingQuote(true);
              setError(null);
              setQuote(null);
              setQuoteId(null);
              setPrivacyAckRequired(false);
              setPrivacyAck(false);
              setReauthToken(null);

              try {
                const res = await requestSwapQuote({
                  swap_type: 'FromZec',
                  swap_mode: 'ExactOutput',
                  input_asset: ZEC_ASSET_ID,
                  input_amount: '', // Not used for ExactOutput
                  output_asset: outputAsset,
                  output_amount: outputAmount.trim(),
                  destination_address: destinationAddress.trim() ? destinationAddress.trim() : null,
                  refund_address: refundAddress.trim() ? refundAddress.trim() : null,
                });

                if ('err' in res) {
                  setError(res.err.message);
                  return;
                }

                setQuoteId(res.ok.quote_id);
                setQuote(res.ok.quote);
              } catch (e) {
                setError(e instanceof Error ? e.message : 'Failed to get quote');
              } finally {
                setSubmittingQuote(false);
              }
            }}
            className="w-full"
          >
            {submittingQuote ? 'Getting quote...' : 'Get payment quote'}
          </Button>
        </CardContent>
      </Card>

      {quote && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Payment Quote</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="rounded-lg bg-muted/50 p-4">
              <div className="text-center">
                <div className="text-sm text-muted-foreground mb-1">You will pay</div>
                <div className="text-2xl font-bold text-primary">
                  {quote.input_amount_formatted} ZEC
                </div>
                <div className="text-sm text-muted-foreground mt-2">Recipient will receive</div>
                <div className="text-lg font-semibold">
                  {quote.output_amount_formatted} {selectedToken?.label ?? ''}
                </div>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4 text-sm">
              <div className="space-y-1">
                <span className="text-muted-foreground">Min. output</span>
                <div className="font-semibold">{quote.min_output_amount}</div>
              </div>
              <div className="space-y-1">
                <span className="text-muted-foreground">Est. time</span>
                <div className="font-semibold">{Math.ceil(quote.time_estimate_secs / 60)} min</div>
              </div>
              <div className="space-y-1 col-span-2">
                <span className="text-muted-foreground flex items-center gap-1">
                  <Clock className="h-3 w-3" />
                  Expires in
                </span>
                <div className={`font-mono font-semibold ${quoteExpired ? 'text-destructive' : ''}`}>
                  {quoteExpired ? 'Expired' : formatCountdown(quote.deadline, nowMs)}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {quoteId && quote && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Confirm Payment</CardTitle>
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

            {privacyAckRequired && (
              <PrivacyWarning acknowledged={privacyAck} onAcknowledgedChange={setPrivacyAck} />
            )}

            <div className="flex gap-3">
              <Button
                disabled={!password.trim() || starting || quoteExpired || (privacyAckRequired && !privacyAck)}
                onClick={async () => {
                  if (!quoteId) return;

                  setStarting(true);
                  setError(null);

                  try {
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
                      })());

                    if (!reauthToken) {
                      setReauthToken(token);
                    }

                    const allow = privacyAckRequired;
                    const startRes = await startSwap({
                      quote_id: quoteId,
                      allow_transparent_interaction: allow,
                      reauth_token: token,
                    });

                    if ('err' in startRes) {
                      if (startRes.err.code === 'PRIVACY_ACK_REQUIRED') {
                        setPrivacyAckRequired(true);
                        setStarting(false);
                        return;
                      }
                      setPassword('');
                      setReauthToken(null);
                      setError(startRes.err.message);
                      setStarting(false);
                      return;
                    }

                      setPassword('');
                      setReauthToken(null);
                      setPrivacyAckRequired(false);
                      setPrivacyAck(false);
                      setStarting(false);
                      navigate('/activity');
                    } catch (e) {
                      setPassword('');
                      setReauthToken(null);
                      setError(e instanceof Error ? e.message : 'Failed to start payment');
                    setStarting(false);
                  }
                }}
                className="flex-1"
              >
                {starting
                  ? 'Processing...'
                  : quoteExpired
                    ? 'Quote expired'
                    : privacyAckRequired
                      ? 'Acknowledge & pay'
                      : 'Pay now'}
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
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}
    </div>
  );
}
