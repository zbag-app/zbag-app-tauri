import { AnimatedQRCode } from '@keystonehq/animated-qr';
import { KeystoneZcashSDK } from '@keystonehq/keystone-sdk';
import { Buffer } from 'buffer';
import { useMemo, useState } from 'react';

function decodeBase64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

const zcashSdk = new KeystoneZcashSDK();

export function AnimatedQRDisplay(props: {
  pcztPayloadBase64: string;
  size?: number;
  intervalMs?: number;
}) {
  const { pcztPayloadBase64, size = 320, intervalMs = 100 } = props;
  const [slowMode, setSlowMode] = useState(false);

  const encoded = useMemo(() => {
    const pcztBytes = decodeBase64ToBytes(pcztPayloadBase64);
    const ur = zcashSdk.generatePczt(Buffer.from(pcztBytes));
    return { cborHex: ur.cbor.toString('hex'), type: ur.type };
  }, [pcztPayloadBase64]);

  return (
    <div style={{ display: 'grid', gap: 8, justifyItems: 'center' }}>
      <AnimatedQRCode
        cbor={encoded.cborHex}
        type={encoded.type}
        options={{ size, interval: slowMode ? 333 : intervalMs }}
      />
      <label style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <input
          type="checkbox"
          checked={slowMode}
          onChange={(e) => setSlowMode(e.currentTarget.checked)}
        />
        <span>Slow QR mode (3 fps)</span>
      </label>
    </div>
  );
}
