import { useEffect, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { Fingerprint, Download, Camera, ArrowLeft, Radio } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { AnimatedQRDisplay } from '../components/signing/AnimatedQRDisplay';
import { FileImport } from '../components/signing/FileImport';
import { QRScanner } from '../components/signing/QRScanner';
import { SigningVerify } from '../components/signing/SigningVerify';
import { finalizeSigning, reauthWallet } from '../services/ipc';

type LocationState = {
  signingRequest: IPC.SigningRequest;
};

function decodeBase64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

export function Signing(props: { walletId: string }) {
  const { walletId } = props;
  const navigate = useNavigate();
  const location = useLocation();

  const [signingRequest] = useState<IPC.SigningRequest | null>(() => {
    const state = location.state as LocationState | null;
    return state?.signingRequest ?? null;
  });
  const summary = signingRequest?.summary ?? null;

  const [password, setPassword] = useState('');
  const [signedPayload, setSignedPayload] = useState<string | null>(null);
  const [scannerOpen, setScannerOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (location.state != null) {
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.pathname, location.state, navigate]);

  useEffect(() => {
    return () => {
      setPassword('');
      setSignedPayload(null);
    };
  }, []);

  const clearSensitive = () => {
    setPassword('');
    setSignedPayload(null);
  };

  if (!signingRequest || !summary) {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <Fingerprint className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Sign with Keystone</h1>
        </div>

        <Card>
          <CardContent className="pt-6">
            <p className="text-muted-foreground">
              Missing signing request. Return to <Link to="/send" className="text-primary hover:underline">Send</Link>.
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <Fingerprint className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Sign with Keystone</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Scan QR Code</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex justify-center p-4 bg-white rounded-none">
            <AnimatedQRDisplay pcztPayloadBase64={signingRequest.pczt_payload} />
          </div>
          <p className="text-sm text-muted-foreground text-center">
            Scan this animated QR with your Keystone to sign the transaction.
          </p>

          <div className="flex gap-3 justify-center flex-wrap">
            <Button
              variant="outline"
              onClick={() => {
                const bytes = decodeBase64ToBytes(signingRequest.pczt_payload);
                const blob = new Blob([bytes], { type: 'application/octet-stream' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = 'zstash-unsigned.pczt';
                a.click();
                URL.revokeObjectURL(url);
              }}
            >
              <Download className="h-4 w-4" />
              Export unsigned PCZT
            </Button>

            <Button
              variant="outline"
              onClick={() => setScannerOpen((v) => !v)}
            >
              <Camera className="h-4 w-4" />
              {scannerOpen ? 'Close scanner' : 'Scan signed QR'}
            </Button>
          </div>
        </CardContent>
      </Card>

      <SigningVerify summary={summary} />

      {scannerOpen && (
        <Card>
          <CardHeader>
            <CardTitle className="text-lg">Scan Signed QR</CardTitle>
          </CardHeader>
          <CardContent>
            <QRScanner
              onScanned={(payloadBase64) => {
                setSignedPayload(payloadBase64);
                setScannerOpen(false);
                setError(null);
              }}
            />
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Import Signed Transaction</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <FileImport
            onImported={(payloadBase64) => {
              setSignedPayload(payloadBase64);
              setScannerOpen(false);
              setError(null);
            }}
          />

          {signedPayload && (
            <div className="rounded-none border border-success/50 bg-success/10 p-3 text-sm text-success">
              Signed payload imported successfully.
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Broadcast Transaction</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="password">Password</Label>
            <Input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.currentTarget.value)}
              disabled={submitting}
              placeholder="Enter your password"
            />
          </div>

          {error && (
            <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}

          <div className="flex gap-3">
            <Button
              disabled={!signedPayload || !password || submitting}
              onClick={async () => {
                if (!signedPayload) return;

                setSubmitting(true);
                setError(null);

                const reauth = await reauthWallet({ wallet_id: walletId, password, purpose: 'Spend' });
                if ('err' in reauth) {
                  setSubmitting(false);
                  setError(reauth.err.message);
                  return;
                }

                const res = await finalizeSigning({
                  signing_request_id: signingRequest.signing_request_id,
                  signed_payload: signedPayload,
                  reauth_token: reauth.ok.reauth_token,
                });

                setSubmitting(false);

                if ('err' in res) {
                  setError(res.err.message);
                  return;
                }

                clearSensitive();
                navigate('/activity');
              }}
              className="flex-1"
            >
              <Radio className="h-4 w-4" />
              {submitting ? 'Broadcasting...' : 'Broadcast transaction'}
            </Button>
            <Button
              variant="outline"
              onClick={() => {
                clearSensitive();
                navigate('/send');
              }}
              disabled={submitting}
            >
              <ArrowLeft className="h-4 w-4" />
              Back
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
