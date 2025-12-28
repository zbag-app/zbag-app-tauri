import { AnimatedQRScanner, Purpose } from '@keystonehq/animated-qr';
import { KeystoneZcashSDK, UR } from '@keystonehq/keystone-sdk';
import { Buffer } from 'buffer';
import { useCallback, useState } from 'react';

const zcashSdk = new KeystoneZcashSDK();

export function QRScanner(props: { onScanned: (payloadBase64: string) => void }) {
  const { onScanned } = props;
  const [error, setError] = useState<string | null>(null);

  const handleScan = useCallback(
    ({ type, cbor }: { type: string; cbor: string }) => {
      try {
        setError(null);
        const ur = new UR(Buffer.from(cbor, 'hex'), type);
        const pcztHex = zcashSdk.parsePczt(ur);
        onScanned(Buffer.from(pcztHex, 'hex').toString('base64'));
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
        urTypes={['zcash-pczt']}
        handleScan={handleScan}
        handleError={(e) => setError(e)}
        options={{ width: 320 }}
      />
      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}
    </div>
  );
}

