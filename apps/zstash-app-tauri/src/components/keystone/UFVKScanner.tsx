import { useCallback, useState } from 'react';
import { AnimatedQRScanner, Purpose } from '@keystonehq/animated-qr';
import { decodeZcashAccountsUrCbor, ZCASH_ACCOUNTS_UR_TYPE } from './zcashAccountsUr';

type Network = 'Mainnet' | 'Testnet';

/**
 * Parse UFVK prefix to detect network.
 * Returns null if the prefix is invalid.
 */
function parseUfvkNetwork(text: string): Network | null {
  const trimmed = text.trim().toLowerCase();
  // Must check uviewtest first (it starts with 'uview' too)
  if (trimmed.startsWith('uviewtest')) return 'Testnet';
  if (trimmed.startsWith('uview')) return 'Mainnet';
  return null;
}

export interface UFVKScanResult {
  ufvk: string;
  seedFingerprint: string | null;
  accountIndex: number;
}

export function UFVKScanner(props: {
  onScanned: (result: UFVKScanResult) => void;
  onCancel?: () => void;
  /** If provided, validates that the scanned UFVK matches this network */
  expectedNetwork?: Network;
}) {
  const { onScanned, onCancel, expectedNetwork } = props;
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [blurEnabled, setBlurEnabled] = useState(true);

  const handleScan = useCallback(
    ({ type, cbor }: { type: string; cbor: string }) => {
      if (type !== ZCASH_ACCOUNTS_UR_TYPE) {
        setError(`Unexpected UR type: ${type}. Expected zcash-accounts.`);
        return;
      }

      try {
        // Convert hex string to Uint8Array (NOT Node.js Buffer)
        const cborBytes = new Uint8Array(
          cbor.match(/.{2}/g)!.map((byte) => parseInt(byte, 16))
        );

        const result = decodeZcashAccountsUrCbor(cborBytes);

        if (result.accounts.length === 0) {
          setError('No accounts found in QR code');
          return;
        }

        const account = result.accounts[0];
        const ufvk = account.ufvk;
        const detectedNetwork = parseUfvkNetwork(ufvk);

        if (!detectedNetwork) {
          setError(`Invalid UFVK format: ${ufvk.substring(0, 20)}...`);
          return;
        }

        // Network mismatch check (only if expectedNetwork is specified)
        if (expectedNetwork && detectedNetwork !== expectedNetwork) {
          setError(
            `Network mismatch: This is a ${detectedNetwork} key, but you're creating a ${expectedNetwork} wallet. ` +
            `Please export the correct key from your Keystone device.`
          );
          return;
        }

        // Validate that account index is present - required for signing
        if (account.index === undefined) {
          setError(
            'QR code missing account index. Cannot determine which key to use for signing. ' +
            'Please re-export the account from your Keystone device.'
          );
          return;
        }

        onScanned({
          ufvk,
          seedFingerprint: result.seedFingerprint,
          accountIndex: account.index,
        });
      } catch (e) {
        setError(`Failed to decode QR: ${e instanceof Error ? e.message : String(e)}`);
      }
    },
    [onScanned, expectedNetwork]
  );

  return (
    <div className="space-y-4">
      <div className="relative rounded-none overflow-hidden bg-muted aspect-square max-w-[320px] mx-auto">
        <AnimatedQRScanner
          purpose={Purpose.SYNC}
          urTypes={[ZCASH_ACCOUNTS_UR_TYPE]}
          handleScan={handleScan}
          handleError={(e) => setError(e)}
          onProgress={(p) => setProgress(p)}
          options={{ width: 320, height: 320, blur: blurEnabled }}
        />
      </div>

      <div className="flex items-center justify-between text-sm">
        <span className="text-muted-foreground">Camera blurred for privacy</span>
        <button
          type="button"
          onClick={() => setBlurEnabled(!blurEnabled)}
          className="text-primary hover:underline"
        >
          {blurEnabled ? 'Show clear view' : 'Enable privacy blur'}
        </button>
      </div>

      {progress > 0 && progress < 100 && (
        <div className="text-sm text-center text-muted-foreground">
          Scanning animated QR: {progress}% complete
        </div>
      )}

      {error && (
        <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      <p className="text-sm text-muted-foreground text-center">
        Point your camera at the animated QR code from your Keystone device
      </p>

      {onCancel && (
        <button
          type="button"
          onClick={onCancel}
          className="w-full text-sm text-muted-foreground hover:text-foreground transition-colors"
        >
          Cancel
        </button>
      )}
    </div>
  );
}
