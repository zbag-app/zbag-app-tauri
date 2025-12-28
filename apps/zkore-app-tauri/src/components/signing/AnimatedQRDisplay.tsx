import { AnimatedQRCode } from '@keystonehq/animated-qr';
import { useMemo, useState } from 'react';
import { encodeZcashPcztUrCbor, ZCASH_PCZT_UR_TYPE } from './zcashPcztUr';

function decodeBase64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export function AnimatedQRDisplay(props: {
  pcztPayloadBase64: string;
  size?: number;
  intervalMs?: number;
}) {
  const { pcztPayloadBase64, size = 320, intervalMs = 100 } = props;
  const [slowMode, setSlowMode] = useState(false);

  const encoded = useMemo(() => {
    const pcztBytes = decodeBase64ToBytes(pcztPayloadBase64);
    const cbor = encodeZcashPcztUrCbor(pcztBytes);
    return { cborHex: Buffer.from(cbor).toString('hex'), type: ZCASH_PCZT_UR_TYPE };
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
