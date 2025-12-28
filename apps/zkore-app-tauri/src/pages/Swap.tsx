import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';
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
      <div style={{ display: 'grid', gap: 12, maxWidth: 560 }}>
        <h1>Swap</h1>
        <div>Swaps are only supported for Mainnet wallets in v1.</div>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gap: 12, maxWidth: 560 }}>
      <h1>Swap</h1>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Swap type</span>
        <select
          value={swapType}
          onChange={(e) => setSwapType(e.currentTarget.value as IPC.SwapType)}
        >
          <option value="ToZec">To ZEC</option>
          <option value="FromZec">From ZEC</option>
        </select>
      </label>

      {swapType === 'FromZec' ? (
        <div style={{ fontSize: 13, opacity: 0.8 }}>
          Swap-from-ZEC is not implemented yet. <Link to="/swap/from-zec">Open page</Link>.
        </div>
      ) : null}

      {swapType === 'ToZec' ? <h2 style={{ margin: 0 }}>To ZEC</h2> : null}

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Input asset</span>
        <select value={inputAsset} onChange={(e) => setInputAsset(e.currentTarget.value)}>
          {supportedTokens
            .filter((t) => t.id !== outputAsset)
            .map((t) => (
              <option key={t.id} value={t.id}>
                {t.label}
              </option>
            ))}
        </select>
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Amount (input asset units)</span>
        <input value={inputAmount} onChange={(e) => setInputAmount(e.currentTarget.value)} />
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Destination ZEC address</span>
        <textarea
          rows={2}
          value={destinationAddress}
          onChange={(e) => setDestinationAddress(e.currentTarget.value)}
          placeholder="u1... / zs... / etc"
          disabled={loadingAddress}
        />
      </label>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <button
        type="button"
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
      >
        {submitting ? 'Requesting quote…' : 'Get quote'}
      </button>
    </div>
  );
}
