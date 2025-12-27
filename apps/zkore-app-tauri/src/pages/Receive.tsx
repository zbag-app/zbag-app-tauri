import { QRCodeSVG } from 'qrcode.react';
import { useEffect, useState } from 'react';
import type * as IPC from '../types/ipc';
import { getReceiveAddress } from '../services/ipc';

export function Receive(props: { activeAccountId: number | null }) {
  const { activeAccountId } = props;
  const [showTransparent, setShowTransparent] = useState(false);
  const [address, setAddress] = useState<IPC.AddressInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      setError(null);
      setAddress(null);
      if (activeAccountId === null) return;

      const res = await getReceiveAddress({
        account_id: activeAccountId,
        address_type: showTransparent ? 'Transparent' : 'ShieldedOnly',
      });
      if (cancelled) return;
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setAddress(res.ok.address);
    }

    run();
    return () => {
      cancelled = true;
    };
  }, [activeAccountId, showTransparent]);

  const copy = async () => {
    if (!address) return;
    await navigator.clipboard.writeText(address.encoded);
  };

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
        <div style={{ display: 'grid', gap: 12 }}>
          <div style={{ display: 'grid', gap: 6 }}>
            <div style={{ fontSize: 14, opacity: 0.8 }}>Address</div>
            <code style={{ wordBreak: 'break-all' }}>{address.encoded}</code>
          </div>
          <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
            <QRCodeSVG value={address.encoded} size={180} />
            <div style={{ display: 'grid', gap: 8 }}>
              <button type="button" onClick={copy}>
                Copy
              </button>
              <div style={{ fontSize: 12, opacity: 0.7 }}>
                Diversifier index: {address.diversifier_index}
              </div>
            </div>
          </div>
        </div>
      ) : (
        <div>{activeAccountId === null ? 'No active account.' : 'Loading…'}</div>
      )}
    </div>
  );
}

