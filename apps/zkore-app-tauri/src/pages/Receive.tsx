import { useEffect, useState } from 'react';
import type * as IPC from '../types/ipc';
import { AddressDisplay } from '../components/wallet/AddressDisplay';
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
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 760 }}>
      <h1>Receive</h1>

      <label style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <input
          type="checkbox"
          checked={showTransparent}
          onChange={(e) => setShowTransparent(e.currentTarget.checked)}
        />
        <span>Show transparent compatibility address</span>
      </label>
      {showTransparent ? (
        <div style={{ fontSize: 14, opacity: 0.85 }}>
          Transparent addresses are receive-only in v1. Funds received to a transparent address must be
          shielded before spending.
        </div>
      ) : null}

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      {address ? (
        <AddressDisplay address={address} />
      ) : (
        <div>{activeAccountId === null ? 'No active account.' : 'Loading…'}</div>
      )}
    </div>
  );
}
