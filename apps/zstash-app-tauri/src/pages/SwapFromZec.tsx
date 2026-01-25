import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { ArrowLeftRight, ArrowLeft, Loader2 } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { ErrorCodes } from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { PrivacyWarning } from '../components/swap/PrivacyWarning';
import {
  ZEC_ASSET_ID,
  DEFAULT_NON_ZEC_ASSET_ID,
  FALLBACK_TOKENS,
  filterSwapTokens,
  sortTokensByPrice,
  getTokenLabel,
  type SupportedToken,
} from '../data/supportedTokens';
import { getReceiveAddress, reauthWallet, requestSwapQuote, startSwap, getSupportedTokens } from '../services/ipc';
import { parseZecToZatoshis } from '../utils/zec';
import { parseSwapError, PRIVACY_ACK_REQUIRED_MESSAGE } from '../utils/swap';
import { formatAtomicAmountForToken } from '../utils/amounts';

export function SwapFromZec(props: { wallet: IPC.WalletInfo; activeAccountId: number | null }) {
  const { wallet, activeAccountId } = props;
  const navigate = useNavigate();

  const [outputAsset, setOutputAsset] = useState(DEFAULT_NON_ZEC_ASSET_ID);
  const [inputAmountZec, setInputAmountZec] = useState('');
  const [destinationAddress, setDestinationAddress] = useState('');
  const [refundAddress, setRefundAddress] = useState('');
  const [loadingRefundAddress, setLoadingRefundAddress] = useState(false);

  const [tokens, setTokens] = useState<SupportedToken[]>([]);
  const [loadingTokens, setLoadingTokens] = useState(true);

  const [quoteId, setQuoteId] = useState<string | null>(null);
  const [quote, setQuote] = useState<IPC.SwapQuote | null>(null);

  const [password, setPassword] = useState('');
  const [reauthToken, setReauthToken] = useState<string | null>(null);
  // FromZec swaps always require transparent interaction (deposit address is transparent)
  // Keep the acknowledgement sticky across re-quotes since the requirement does not change.
  const [privacyAck, setPrivacyAck] = useState(false);

  const [submittingQuote, setSubmittingQuote] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const lastQuoteInputsRef = useRef<{
    outputAsset: string;
    inputAmountZatoshis: string | null;
    destinationAddress: string;
    refundAddress: string;
  } | null>(null);

  const outputAssetTrimmed = outputAsset.trim();
  const inputAmountZecTrimmed = inputAmountZec.trim();
  const destinationAddressTrimmed = destinationAddress.trim();
  const refundAddressTrimmed = refundAddress.trim();

  const parsedAmount = useMemo<{ zatoshis: string | null; error: string | null }>(() => {
    if (!inputAmountZecTrimmed) return { zatoshis: null, error: null };
    const res = parseZecToZatoshis(inputAmountZecTrimmed);
    if ('err' in res) return { zatoshis: null, error: res.err };
    return { zatoshis: res.ok, error: null };
  }, [inputAmountZecTrimmed]);

  const amountError = parsedAmount.error;
  const inputAmountZatoshis = parsedAmount.zatoshis;

  // Filter and sort tokens for display
  const availableTokens = useMemo(() => {
    const filtered = filterSwapTokens(tokens);
    return sortTokensByPrice(filtered);
  }, [tokens]);

  const canQuote = useMemo(() => {
    if (wallet.network !== 'Mainnet') return false;
    if (activeAccountId == null) return false;
    if (!outputAssetTrimmed) return false;
    if (!inputAmountZatoshis) return false;
    if (!destinationAddressTrimmed) return false;
    if (!refundAddressTrimmed) return false;
    return true;
  }, [
    wallet.network,
    activeAccountId,
    outputAssetTrimmed,
    inputAmountZatoshis,
    destinationAddressTrimmed,
    refundAddressTrimmed,
  ]);

  // Load supported tokens from API
  useEffect(() => {
    let cancelled = false;

    async function loadTokens() {
      setLoadingTokens(true);
      const res = await getSupportedTokens();
      if (cancelled) return;
      setLoadingTokens(false);

      if ('err' in res) {
        // Fall back to static list on error
        setTokens(FALLBACK_TOKENS);
        return;
      }

      if (res.ok.tokens.length === 0) {
        // Fall back if API returns empty
        setTokens(FALLBACK_TOKENS);
        return;
      }

      setTokens(res.ok.tokens);
    }

    loadTokens();
    return () => {
      cancelled = true;
    };
  }, []);

  // Auto-populate refund address from wallet's shielded address
  useEffect(() => {
    let cancelled = false;

    async function loadRefundAddress() {
      if (wallet.network !== 'Mainnet') return;
      if (activeAccountId == null) return;

      setLoadingRefundAddress(true);
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

        setRefundAddress(res.ok.address.encoded);
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : 'Failed to load refund address');
      } finally {
        if (!cancelled) setLoadingRefundAddress(false);
      }
    }

    loadRefundAddress();
    return () => {
      cancelled = true;
    };
  }, [wallet.network, activeAccountId]);

  // Validate selected asset after tokens load (PR review: race condition fix)
  useEffect(() => {
    if (loadingTokens || availableTokens.length === 0) return;
    const selectedExists = availableTokens.some((t) => t.asset_id === outputAsset);
    if (!selectedExists) {
      setOutputAsset(availableTokens[0].asset_id);
    }
  }, [loadingTokens, availableTokens, outputAsset]);

  // Clear errors when quote inputs change (clear on any keystroke, even if canonical values are unchanged).
  useEffect(() => {
    setError(null);
  }, [outputAsset, inputAmountZec, destinationAddress, refundAddress]);

  // Invalidate quotes when canonical quote inputs change.
  useEffect(() => {
    const nextInputs = {
      outputAsset: outputAssetTrimmed,
      inputAmountZatoshis,
      destinationAddress: destinationAddressTrimmed,
      refundAddress: refundAddressTrimmed,
    };
    const prevInputs = lastQuoteInputsRef.current;

    if (!prevInputs) {
      lastQuoteInputsRef.current = nextInputs;
      return;
    }

    const inputsChanged =
      prevInputs.outputAsset !== nextInputs.outputAsset ||
      prevInputs.inputAmountZatoshis !== nextInputs.inputAmountZatoshis ||
      prevInputs.destinationAddress !== nextInputs.destinationAddress ||
      prevInputs.refundAddress !== nextInputs.refundAddress;
    if (!inputsChanged) return;

    if (submittingQuote || starting) {
      // Do not advance the ref while an async quote/start is in-flight.
      // This ensures any mid-flight input changes are still detected and will invalidate stale quote state
      // immediately after the in-flight operation completes.
      return;
    }

    lastQuoteInputsRef.current = nextInputs;

    setQuote(null);
    setQuoteId(null);
    setReauthToken(null);
    setPassword('');
    // Intentionally NOT resetting privacyAck; FromZec swaps always require transparent interaction.
  }, [
    destinationAddressTrimmed,
    inputAmountZatoshis,
    outputAssetTrimmed,
    refundAddressTrimmed,
    starting,
    submittingQuote,
  ]);

  // Format min output amount with token symbol
  const formattedMinOutput = useMemo(() => {
    if (!quote) return null;
    return formatAtomicAmountForToken(quote.min_output_amount, quote.output_asset);
  }, [quote]);
  const minOutputIsRaw = formattedMinOutput?.isRaw ?? false;

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
            {loadingTokens ? (
              <div className="flex h-9 items-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="h-4 w-4 animate-spin" />
                Loading tokens...
              </div>
            ) : (
              <select
                id="outputAsset"
                value={outputAsset}
                onChange={(e) => setOutputAsset(e.currentTarget.value)}
                disabled={submittingQuote || starting}
                className="flex h-9 w-full rounded-none border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                {availableTokens.map((t) => (
                  <option key={t.asset_id} value={t.asset_id}>
                    {getTokenLabel(t)}
                  </option>
                ))}
              </select>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor="inputAmount">Amount (ZEC)</Label>
            <Input
              id="inputAmount"
              value={inputAmountZec}
              onChange={(e) => setInputAmountZec(e.currentTarget.value)}
              inputMode="decimal"
              placeholder="0.0"
              disabled={submittingQuote || starting}
            />
            <p className="text-xs text-muted-foreground">Up to 8 decimal places</p>
            {amountError && <p className="text-xs text-destructive">{amountError}</p>}
          </div>

          <div className="space-y-2">
            <Label htmlFor="destinationAddress">Destination address (target asset chain)</Label>
            <Input
              id="destinationAddress"
              value={destinationAddress}
              onChange={(e) => setDestinationAddress(e.currentTarget.value)}
              placeholder="Paste the destination address for the target asset"
              className="font-mono"
              disabled={submittingQuote || starting}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="refundAddress">Refund address (ZEC)</Label>
            <Input
              id="refundAddress"
              value={refundAddress}
              onChange={(e) => setRefundAddress(e.currentTarget.value)}
              placeholder="Your ZEC address for refunds"
              disabled={loadingRefundAddress || submittingQuote || starting}
              className="font-mono"
            />
          </div>

          <PrivacyWarning
            acknowledged={privacyAck}
            onAcknowledgedChange={setPrivacyAck}
            disabled={submittingQuote || starting}
          />

          <Button
            disabled={!canQuote || submittingQuote || starting || loadingTokens}
            onClick={async () => {
              if (!canQuote || !inputAmountZatoshis) return;
              setSubmittingQuote(true);
              setError(null);
              setQuote(null);
              setQuoteId(null);
              setReauthToken(null);
              setPassword('');

              try {
                const res = await requestSwapQuote({
                  swap_type: 'FromZec',
                  input_asset: ZEC_ASSET_ID,
                  input_amount: inputAmountZatoshis,
                  output_asset: outputAssetTrimmed,
                  destination_address: destinationAddressTrimmed ? destinationAddressTrimmed : null,
                  refund_address: refundAddressTrimmed ? refundAddressTrimmed : null,
                });

                if ('err' in res) {
                  setError(parseSwapError(res.err.message));
                  return;
                }

                setQuoteId(res.ok.quote_id);
                setQuote(res.ok.quote);
              } catch (e) {
                setError(e instanceof Error ? e.message : 'Failed to request quote');
              } finally {
                setSubmittingQuote(false);
              }
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
                <span className="text-muted-foreground">{minOutputIsRaw ? 'Min. output (raw)' : 'Min. output'}</span>
                <div className="font-semibold">{formattedMinOutput?.value}</div>
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

                  setStarting(true);
                  setError(null);

                  try {
                    let token = reauthToken;
                    if (!token) {
                      const res = await reauthWallet({
                        wallet_id: wallet.id,
                        password,
                        purpose: 'Spend',
                      });
                      if ('err' in res) {
                        setError(res.err.message);
                        return;
                      }

                      token = res.ok.reauth_token;
                      setReauthToken(token);
                    }

                    const startRes = await startSwap({
                      quote_id: quoteId,
                      allow_transparent_interaction: privacyAck,
                      reauth_token: token,
                    });

                    if ('err' in startRes) {
                      const message =
                        startRes.err.code === ErrorCodes.PRIVACY_ACK_REQUIRED
                          ? PRIVACY_ACK_REQUIRED_MESSAGE
                          : parseSwapError(startRes.err.message);
                      setError(message);
                      return;
                    }

                    setPassword('');
                    setReauthToken(null);
                    navigate('/activity');
                  } catch (e) {
                    setError(e instanceof Error ? e.message : 'Failed to start swap');
                  } finally {
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
