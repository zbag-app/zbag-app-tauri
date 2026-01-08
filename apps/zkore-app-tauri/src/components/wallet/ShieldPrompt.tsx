import { useEffect, useMemo, useRef, useState } from 'react';
import type * as IPC from '../../types/ipc';
import { reauthWallet, shieldFunds } from '../../services/ipc';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { formatZatoshisToZec } from '../../utils/zec';

type InsufficientFeeDetails = {
  required_minimum_zatoshis?: unknown;
  available_zatoshis?: unknown;
  estimated_fee_zatoshis?: unknown;
};

function formatMaybeString(value: unknown): string | null {
  if (typeof value === 'string') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return String(value);
  return null;
}

export function ShieldPrompt(props: {
  walletId: string;
  accountId: number;
  transparentTotal: string;
  disabled?: boolean;
  onShielded?: () => void;
}) {
  const { walletId, accountId, transparentTotal, disabled, onShielded } = props;

  const [open, setOpen] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const loadingRef = useRef(false);
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<IPC.IpcError | null>(null);
  const [result, setResult] = useState<IPC.ShieldFundsResponse | null>(null);

  useEffect(() => {
    loadingRef.current = loading;
  }, [loading]);

  useFocusTrap(dialogRef, open);
  useKeyboardShortcuts('esc', () => {
    if (!loadingRef.current) setOpen(false);
  }, open);

  useEffect(() => {
    if (!open) {
      setPassword('');
      setLoading(false);
      setError(null);
      setResult(null);
    }
  }, [open]);

  const insufficientFeeDetails = useMemo((): InsufficientFeeDetails | null => {
    if (!error?.details) return null;
    return error.details as InsufficientFeeDetails;
  }, [error]);

  const submit = async () => {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      const tokenRes = await reauthWallet({ wallet_id: walletId, password, purpose: 'Spend' });
      if ('err' in tokenRes) {
        setError(tokenRes.err);
        return;
      }

      const shieldRes = await shieldFunds({
        account_id: accountId,
        consolidate: true,
        reauth_token: tokenRes.ok.reauth_token,
      });
      if ('err' in shieldRes) {
        setError(shieldRes.err);
        return;
      }

      setResult(shieldRes.ok);
      setPassword('');
      onShielded?.();
    } finally {
      setLoading(false);
    }
  };

  const requiredMinimum = formatMaybeString(insufficientFeeDetails?.required_minimum_zatoshis);
  const available = formatMaybeString(insufficientFeeDetails?.available_zatoshis);
  const estimatedFee = formatMaybeString(insufficientFeeDetails?.estimated_fee_zatoshis);

  return (
    <div style={{ display: 'inline-flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
      <button type="button" onClick={() => setOpen(true)} disabled={disabled}>
        Shield now
      </button>

      {open ? (
        <div
          role="dialog"
          aria-modal="true"
          aria-label="Shield transparent funds"
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.45)',
            display: 'grid',
            placeItems: 'center',
            padding: 16,
          }}
        >
          <div
            ref={dialogRef}
            style={{ background: 'white', borderRadius: 12, padding: 16, maxWidth: 720, width: '100%' }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
              <h2 style={{ margin: 0 }}>Shield and consolidate</h2>
              <button
                type="button"
                onClick={() => setOpen(false)}
                disabled={loading}
                aria-label="Close shield dialog"
              >
                Close
              </button>
            </div>

            <div style={{ display: 'grid', gap: 10, marginTop: 12 }}>
              <div style={{ fontSize: 14, opacity: 0.9 }}>
                Transparent funds are receive-only until shielded. This action sweeps all spendable transparent
                funds into Orchard. Fees are deducted from transparent inputs.
              </div>

              <div style={{ display: 'grid', gap: 6 }}>
                <div style={{ display: 'grid', gridTemplateColumns: '200px 1fr', gap: 6 }}>
                  <div style={{ opacity: 0.8 }}>Transparent total</div>
                  <div>{formatZatoshisToZec(transparentTotal)} ZEC</div>
                </div>
                <div style={{ fontSize: 12, opacity: 0.75 }}>
                  If there are too many UTXOs to fit, shielding batches into multiple transactions.
                </div>
              </div>

              {result ? (
                <div style={{ padding: 12, border: '1px solid #16a34a', borderRadius: 8, background: '#f0fdf4' }}>
                  <strong>Shielding started</strong>
                  <div style={{ marginTop: 6, display: 'grid', gap: 6 }}>
                    <div style={{ display: 'grid', gridTemplateColumns: '140px 1fr', gap: 6 }}>
                      <div style={{ opacity: 0.8 }}>Txid</div>
                      <code style={{ wordBreak: 'break-all' }}>{result.txid}</code>
                      <div style={{ opacity: 0.8 }}>Fee</div>
                      <div>{formatZatoshisToZec(result.fee)} ZEC</div>
                    </div>
                    <div style={{ fontSize: 12, opacity: 0.75 }}>
                      Additional shielding transactions (if any) are visible in Activity.
                    </div>
                  </div>
                </div>
              ) : (
                <form
                  style={{ display: 'grid', gap: 10 }}
                  onSubmit={(e) => {
                    e.preventDefault();
                    void submit();
                  }}
                >
                  <label style={{ display: 'grid', gap: 4, maxWidth: 420 }}>
                    <span>Wallet password</span>
                    <input
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.currentTarget.value)}
                      disabled={loading}
                    />
                  </label>

                  {error ? (
                    <div style={{ display: 'grid', gap: 6 }}>
                      <div style={{ color: 'crimson' }}>
                        {error.code}: {error.message}
                      </div>

                      {error.code === 'E3001' && (requiredMinimum || available || estimatedFee) ? (
                        <div style={{ padding: 12, border: '1px solid #ef4444', borderRadius: 8 }}>
                          <strong>Not enough transparent funds to pay the shielding fee</strong>
                          <div style={{ marginTop: 8, display: 'grid', gap: 6 }}>
                            {requiredMinimum ? <div>Required minimum: {formatZatoshisToZec(requiredMinimum)} ZEC</div> : null}
                            {available ? <div>Available: {formatZatoshisToZec(available)} ZEC</div> : null}
                            {estimatedFee ? <div>Estimated fee: {formatZatoshisToZec(estimatedFee)} ZEC</div> : null}
                          </div>
                          <div style={{ marginTop: 8, fontSize: 12, opacity: 0.85 }}>
                            Acquire a minimal amount of additional transparent ZEC and retry.
                          </div>
                        </div>
                      ) : null}
                    </div>
                  ) : null}

                  <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
                    <button type="submit" disabled={!password || loading}>
                      {loading ? 'Shielding…' : 'Confirm & Shield'}
                    </button>
                    <button type="button" onClick={() => setOpen(false)} disabled={loading}>
                      Cancel
                    </button>
                  </div>
                </form>
              )}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
