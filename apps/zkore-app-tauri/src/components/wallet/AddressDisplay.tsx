import { QRCodeSVG } from 'qrcode.react';
import type * as IPC from '../../types/ipc';

export function AddressDisplay(props: { address: IPC.AddressInfo }) {
  const { address } = props;

  const copy = async () => {
    await navigator.clipboard.writeText(address.encoded);
  };

  return (
    <div style={{ display: 'grid', gap: 12 }}>
      <div style={{ display: 'grid', gap: 6 }}>
        <div style={{ fontSize: 14, opacity: 0.8 }}>Address</div>
        <code style={{ wordBreak: 'break-all' }}>{address.encoded}</code>
      </div>

      <div style={{ display: 'flex', gap: 16, alignItems: 'center', flexWrap: 'wrap' }}>
        <QRCodeSVG value={address.encoded} size={240} />
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
  );
}

