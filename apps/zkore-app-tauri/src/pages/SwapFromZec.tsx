import { useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { PrivacyWarning } from '../components/swap/PrivacyWarning';
import { supportedTokens } from '../data/supportedTokens';
import { reauthWallet, requestSwapQuote, startSwap } from '../services/ipc';
import type * as IPC from '../types/ipc';

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
      <div style={{ display: 'grid', gap: 12, maxWidth: 560 }}>
        <h1>Swap From ZEC</h1>
        <div>Swaps are only supported for Mainnet wallets in v1.</div>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gap: 12, maxWidth: 760 }}>
      <h1>Swap From ZEC</h1>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Target asset</span>
        <select value={outputAsset} onChange={(e) => setOutputAsset(e.currentTarget.value)}>
          {supportedTokens
            .filter((t) => t.id !== 'zcash:mainnet:native')
            .map((t) => (
              <option key={t.id} value={t.id}>
                {t.label}
              </option>
            ))}
        </select>
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Amount (zatoshis)</span>
        <input value={inputAmountZat} onChange={(e) => setInputAmountZat(e.currentTarget.value)} />
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Destination address (target asset chain)</span>
        <textarea
          rows={2}
          value={destinationAddress}
          onChange={(e) => setDestinationAddress(e.currentTarget.value)}
          placeholder="Paste the destination address for the target asset"
        />
      </label>

      <button
        type="button"
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
      >
        {submittingQuote ? 'Requesting quote…' : 'Get quote'}
      </button>

      {quote ? (
        <div style={{ display: 'grid', gap: 6, padding: 12, border: '1px solid #e5e7eb', borderRadius: 8 }}>
          <div>
            {quote.input_amount} {quote.input_asset} → {quote.output_amount} {quote.output_asset}
          </div>
          <div style={{ fontSize: 13, opacity: 0.8 }}>
            Fee: {quote.fee_amount} {quote.fee_asset}
          </div>
          <div style={{ fontSize: 13, opacity: 0.8 }}>Rate: {quote.rate}</div>
        </div>
      ) : null}

      {quoteId && quote ? (
        <div style={{ display: 'grid', gap: 12 }}>
          <label style={{ display: 'grid', gap: 4, maxWidth: 420 }}>
            <span>Password</span>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.currentTarget.value)}
              disabled={starting}
            />
          </label>

          {privacyAckRequired ? (
            <PrivacyWarning acknowledged={privacyAck} onAcknowledgedChange={setPrivacyAck} />
          ) : null}

          <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
            <button
              type="button"
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
            >
              {starting ? 'Starting…' : privacyAckRequired ? 'Acknowledge & start swap' : 'Start swap'}
            </button>
            <Link to="/swap">Back</Link>
          </div>
        </div>
      ) : null}

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}
    </div>
  );
}
