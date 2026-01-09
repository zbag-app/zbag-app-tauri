import { useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { ArrowLeftRight, ArrowLeft } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { PrivacyWarning } from '../components/swap/PrivacyWarning';
import { supportedTokens } from '../data/supportedTokens';
import { reauthWallet, requestSwapQuote, startSwap } from '../services/ipc';

export function SwapFromZec(props: { wallet: IPC.WalletInfo }) {
  const { wallet } = props;
  const navigate = useNavigate();

  const [outputAsset, setOutputAsset] = useState('near:mainnet:native');
  const [inputAmountZat, setInputAmountZat] = useState('');
  const [destinationAddress, setDestinationAddress] = useState('');

  const [quoteId, setQuoteId] = useState<string | null>(null);
  const [quote, setQuote] = useState<IPC.SwapQuote | null>(null);

  const [password, setPassword] = useState('');
  const [reauthToken, setReauthToken] = useState<string | null>(null);
  const [privacyAckRequired, setPrivacyAckRequired] = useState(false);
  const [privacyAck, setPrivacyAck] = useState(false);

  const [submittingQuote, setSubmittingQuote] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canQuote = useMemo(() => {
    if (wallet.network !== 'Mainnet') return false;
    if (!outputAsset.trim()) return false;
    if (!inputAmountZat.trim()) return false;
    if (!destinationAddress.trim()) return false;
    return true;
  }, [wallet.network, outputAsset, inputAmountZat, destinationAddress]);

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
              className="flex h-9 w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {supportedTokens
                .filter((t) => t.id !== 'zcash:mainnet:native')
                .map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.label}
                  </option>
                ))}
            </select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="inputAmount">Amount (zatoshis)</Label>
            <Input
              id="inputAmount"
              value={inputAmountZat}
              onChange={(e) => setInputAmountZat(e.currentTarget.value)}
              placeholder="Enter amount in zatoshis"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="destinationAddress">Destination address (target asset chain)</Label>
            <textarea
              id="destinationAddress"
              rows={2}
              value={destinationAddress}
              onChange={(e) => setDestinationAddress(e.currentTarget.value)}
              placeholder="Paste the destination address for the target asset"
              className="flex w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring font-mono"
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

              const res = await requestSwapQuote({
                swap_type: 'FromZec',
                input_asset: 'zcash:mainnet:native',
                input_amount: inputAmountZat,
                output_asset: outputAsset,
                destination_address: destinationAddress.trim() ? destinationAddress.trim() : null,
                refund_address: null,
              });
              setSubmittingQuote(false);

              if ('err' in res) {
                setError(res.err.message);
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

            {privacyAckRequired && (
              <PrivacyWarning acknowledged={privacyAck} onAcknowledgedChange={setPrivacyAck} />
            )}

            <div className="flex gap-3">
              <Button
                disabled={!password.trim() || starting || (privacyAckRequired && !privacyAck)}
                onClick={async () => {
                  if (!quoteId) return;

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

                    const allow = privacyAckRequired ? true : false;
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
                      setError(startRes.err.message);
                      setStarting(false);
                      return;
                    }

                    setPassword('');
                    setReauthToken(null);
                    setPrivacyAckRequired(false);
                    setPrivacyAck(false);
                    navigate('/activity');
                  } catch (e) {
                    setError(e instanceof Error ? e.message : 'Failed to start swap');
                    setStarting(false);
                  }
                }}
                className="flex-1"
              >
                {starting ? 'Starting...' : privacyAckRequired ? 'Acknowledge & start swap' : 'Start swap'}
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
