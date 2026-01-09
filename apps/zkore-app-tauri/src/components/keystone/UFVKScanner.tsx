import { useEffect, useRef, useState, useCallback } from 'react';
import { BrowserMultiFormatReader, NotFoundException } from '@zxing/library';

function isValidUfvk(text: string): boolean {
  // UFVKs start with "uview" (mainnet) or "uivk" (testnet)
  const trimmed = text.trim().toLowerCase();
  return trimmed.startsWith('uview') || trimmed.startsWith('uivk');
}

export function UFVKScanner(props: {
  onScanned: (ufvk: string) => void;
  onCancel?: () => void;
}) {
  const { onScanned, onCancel } = props;
  const videoRef = useRef<HTMLVideoElement>(null);
  const readerRef = useRef<BrowserMultiFormatReader | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);

  const startScanning = useCallback(async () => {
    if (!videoRef.current) return;

    try {
      setError(null);
      setScanning(true);

      const reader = new BrowserMultiFormatReader();
      readerRef.current = reader;

      await reader.decodeFromVideoDevice(
        null, // Use default camera
        videoRef.current,
        (result, err) => {
          if (result) {
            const text = result.getText();
            if (isValidUfvk(text)) {
              reader.reset();
              setScanning(false);
              onScanned(text.trim());
            } else {
              setError('QR code does not contain a valid UFVK. Expected format: uview... or uivk...');
            }
          }
          if (err && !(err instanceof NotFoundException)) {
            console.error('QR scan error:', err);
          }
        }
      );
    } catch (e) {
      setScanning(false);
      if (e instanceof Error) {
        if (e.name === 'NotAllowedError') {
          setError('Camera access denied. Please allow camera access to scan QR codes.');
        } else if (e.name === 'NotFoundError') {
          setError('No camera found. Please connect a camera to scan QR codes.');
        } else {
          setError(e.message);
        }
      } else {
        setError('Failed to access camera');
      }
    }
  }, [onScanned]);

  useEffect(() => {
    startScanning();

    return () => {
      if (readerRef.current) {
        readerRef.current.reset();
        readerRef.current = null;
      }
    };
  }, [startScanning]);

  return (
    <div className="space-y-4">
      <div className="relative rounded-lg overflow-hidden bg-muted aspect-square max-w-[320px] mx-auto">
        <video
          ref={videoRef}
          className="w-full h-full object-cover"
          playsInline
          muted
        />
        {scanning && (
          <div className="absolute inset-0 border-2 border-primary/50 rounded-lg pointer-events-none">
            <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-32 h-32 border-2 border-primary rounded-lg" />
          </div>
        )}
      </div>

      {error && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      <p className="text-sm text-muted-foreground text-center">
        Point your camera at the UFVK QR code from your Keystone device
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
