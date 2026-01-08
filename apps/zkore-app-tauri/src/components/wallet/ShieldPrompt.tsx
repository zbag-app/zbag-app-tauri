import { useEffect, useMemo, useRef, useState } from 'react';
import { Shield, X } from 'lucide-react';
import type * as IPC from '../../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../ui/card';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Label } from '../ui/label';
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
    <>
      <Button
        variant="secondary"
        className="w-full flex-col h-auto py-3 gap-1"
        onClick={() => setOpen(true)}
        disabled={disabled}
      >
        <Shield className="h-5 w-5" />
        <span className="text-xs">Shield</span>
      </Button>

      {open && (
        <div
          role="dialog"
          aria-modal="true"
          aria-label="Shield transparent funds"
          className="fixed inset-0 z-50 bg-background/80 backdrop-blur-sm flex items-center justify-center p-4"
        >
          <Card ref={dialogRef} className="w-full max-w-lg animate-[scale-in_0.2s_ease-out]">
            <CardHeader className="flex flex-row items-center justify-between">
              <CardTitle className="text-lg">Shield and Consolidate</CardTitle>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setOpen(false)}
                disabled={loading}
                aria-label="Close shield dialog"
              >
                <X className="h-4 w-4" />
              </Button>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm text-muted-foreground">
                Transparent funds are receive-only until shielded. This action sweeps all spendable transparent
                funds into Orchard. Fees are deducted from transparent inputs.
              </p>

              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">Transparent total</span>
                  <span className="font-semibold">{formatZatoshisToZec(transparentTotal)} ZEC</span>
                </div>
                <p className="text-xs text-muted-foreground">
                  If there are too many UTXOs to fit, shielding batches into multiple transactions.
                </p>
              </div>

              {result ? (
                <div className="rounded-lg border border-success/50 bg-success/10 p-4 space-y-3">
                  <h4 className="font-semibold text-success">Shielding Started</h4>
                  <div className="space-y-2 text-sm">
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Txid</span>
                      <code className="text-xs font-mono break-all max-w-[200px]">{result.txid}</code>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Fee</span>
                      <span>{formatZatoshisToZec(result.fee)} ZEC</span>
                    </div>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    Additional shielding transactions (if any) are visible in Activity.
                  </p>
                </div>
              ) : (
                <form
                  className="space-y-4"
                  onSubmit={(e) => {
                    e.preventDefault();
                    void submit();
                  }}
                >
                  <div className="space-y-2">
                    <Label htmlFor="shieldPassword">Wallet password</Label>
                    <Input
                      id="shieldPassword"
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.currentTarget.value)}
                      disabled={loading}
                      placeholder="Enter your password"
                    />
                  </div>

                  {error && (
                    <div className="space-y-2">
                      <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                        {error.code}: {error.message}
                      </div>

                      {error.code === 'E3001' && (requiredMinimum || available || estimatedFee) && (
                        <div className="rounded-lg border border-destructive/50 bg-destructive/5 p-3 space-y-2">
                          <h5 className="font-semibold text-sm">Not enough transparent funds to pay the shielding fee</h5>
                          <div className="text-xs space-y-1">
                            {requiredMinimum && <div>Required minimum: {formatZatoshisToZec(requiredMinimum)} ZEC</div>}
                            {available && <div>Available: {formatZatoshisToZec(available)} ZEC</div>}
                            {estimatedFee && <div>Estimated fee: {formatZatoshisToZec(estimatedFee)} ZEC</div>}
                          </div>
                          <p className="text-xs text-muted-foreground">
                            Acquire a minimal amount of additional transparent ZEC and retry.
                          </p>
                        </div>
                      )}
                    </div>
                  )}

                  <div className="flex gap-3">
                    <Button type="submit" disabled={!password || loading} className="flex-1">
                      {loading ? 'Shielding...' : 'Confirm & Shield'}
                    </Button>
                    <Button type="button" variant="outline" onClick={() => setOpen(false)} disabled={loading}>
                      Cancel
                    </Button>
                  </div>
                </form>
              )}
            </CardContent>
          </Card>
        </div>
      )}
    </>
  );
}
