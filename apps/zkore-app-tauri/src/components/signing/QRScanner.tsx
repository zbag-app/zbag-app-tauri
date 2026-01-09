import { AnimatedQRScanner, Purpose } from '@keystonehq/animated-qr';
import { useCallback, useState } from 'react';
import { decodeZcashPcztUrCbor, ZCASH_PCZT_UR_TYPE } from './zcashPcztUr';

export function QRScanner(props: { onScanned: (payloadBase64: string) => void }) {
  const { onScanned } = props;
  const [error, setError] = useState<string | null>(null);
  const [blurEnabled, setBlurEnabled] = useState(true);
  const [progress, setProgress] = useState(0);

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
        onProgress={(percent) => setProgress(percent)}
        options={{ width: 320, blur: blurEnabled }}
      />
      {progress > 0 && progress < 100 && (
        <div className="w-full max-w-[320px]">
          <div className="h-2 bg-muted rounded-full overflow-hidden">
            <div
              className="h-full bg-primary transition-all duration-200"
              style={{ width: `${progress}%` }}
            />
          </div>
          <p className="text-xs text-muted-foreground mt-1 text-center">
            Scanning: {progress}% - Keep camera pointed at QR
          </p>
        </div>
      )}
      <div className="flex items-center justify-between text-sm w-full max-w-[320px]">
        <span className="text-muted-foreground">Camera blurred for privacy</span>
        <button
          type="button"
          onClick={() => setBlurEnabled(!blurEnabled)}
          className="text-primary hover:underline"
        >
          {blurEnabled ? 'Show clear view' : 'Enable privacy blur'}
        </button>
      </div>
      {error ? <div className="text-sm text-destructive">{error}</div> : null}
    </div>
  );
}
