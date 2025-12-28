import { AnimatedQRScanner, Purpose } from '@keystonehq/animated-qr';
import { useCallback, useState } from 'react';
import { decodeZcashPcztUrCbor, ZCASH_PCZT_UR_TYPE } from './zcashPcztUr';

export function QRScanner(props: { onScanned: (payloadBase64: string) => void }) {
  const { onScanned } = props;
  const [error, setError] = useState<string | null>(null);

  const handleScan = useCallback(
    ({ type, cbor }: { type: string; cbor: string }) => {
      try {
        setError(null);
        if (type !== ZCASH_PCZT_UR_TYPE) {
          throw new Error(`Unexpected UR type: ${type}`);
        }
        const pcztBytes = decodeZcashPcztUrCbor(Buffer.from(cbor, 'hex'));
        onScanned(Buffer.from(pcztBytes).toString('base64'));
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Scan failed');
      }
    },
    [onScanned]
  );

  return (
    <div style={{ display: 'grid', gap: 8, justifyItems: 'center' }}>
      <AnimatedQRScanner
        purpose={Purpose.SIGN}
        urTypes={[ZCASH_PCZT_UR_TYPE]}
        handleScan={handleScan}
        handleError={(e) => setError(e)}
        options={{ width: 320 }}
      />
      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}
    </div>
  );
}
