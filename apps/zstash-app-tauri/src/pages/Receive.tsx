import { useEffect, useState } from 'react';
import { Download, AlertTriangle } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { AddressDisplay } from '../components/wallet/AddressDisplay';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { getReceiveAddress } from '../services/ipc';

export function Receive(props: { activeAccountId: number | null }) {
  const { activeAccountId } = props;
  const [showTransparent, setShowTransparent] = useState(false);
  const [shieldedAddress, setShieldedAddress] = useState<IPC.AddressInfo | null>(null);
  const [transparentAddress, setTransparentAddress] = useState<IPC.AddressInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      setError(null);
      setShieldedAddress(null);
      setTransparentAddress(null);
      if (activeAccountId === null) return;

      const [shieldedRes, transparentRes] = await Promise.all([
        getReceiveAddress({
          account_id: activeAccountId,
          address_type: 'ShieldedOnly',
        }),
        getReceiveAddress({
          account_id: activeAccountId,
          address_type: 'Transparent',
        }),
      ]);

      if (cancelled) return;
      if ('err' in shieldedRes) {
        setError(shieldedRes.err.message);
        return;
      }
      if ('err' in transparentRes) {
        setError(transparentRes.err.message);
        return;
      }

      setShieldedAddress(shieldedRes.ok.address);
      setTransparentAddress(transparentRes.ok.address);
    }

    run();
    return () => {
      cancelled = true;
    };
  }, [activeAccountId]);

  const address = showTransparent ? transparentAddress : shieldedAddress;

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <Download className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Receive</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Your Address</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <label className="flex items-center gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={showTransparent}
              onChange={(e) => setShowTransparent(e.currentTarget.checked)}
              className="rounded border-border h-4 w-4 accent-primary"
            />
            <span className="text-sm">Show transparent compatibility address</span>
          </label>

          {showTransparent && (
            <div className="flex items-start gap-3 rounded-lg border border-warning/50 bg-warning/5 p-3">
              <AlertTriangle className="h-4 w-4 text-warning shrink-0 mt-0.5" />
              <p className="text-sm text-muted-foreground">
                Transparent addresses are receive-only. Funds received to a transparent address must be
                shielded before spending.
              </p>
            </div>
          )}

          {error && (
            <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}

          {address ? (
            <AddressDisplay address={address} />
          ) : (
            <div className="text-muted-foreground">
              {activeAccountId === null ? 'No active account' : 'Loading...'}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
