import { useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import type * as IPC from '../types/ipc';
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
  const state = location.state as LocationState | null;

  const signingRequest = state?.signingRequest ?? null;
  const summary = signingRequest?.summary ?? null;

  const [password, setPassword] = useState('');
  const [signedPayload, setSignedPayload] = useState<string | null>(null);
  const [scannerOpen, setScannerOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const clearSensitive = () => {
    setPassword('');
    setSignedPayload(null);
  };

  if (!signingRequest || !summary) {
    return (
      <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 760 }}>
        <h1>Sign with Keystone</h1>
        <div>
          Missing signing request. Return to <Link to="/send">Send</Link>.
        </div>
      </div>
    );
  }

  return (
    <div
      style={{
        display: 'grid',
        gap: 16,
        padding: 16,
        minHeight: '100vh',
        placeContent: 'start center',
      }}
    >
      <h1>Sign with Keystone</h1>

      <div style={{ display: 'grid', gap: 12, justifyItems: 'center' }}>
        <AnimatedQRDisplay pcztPayloadBase64={signingRequest.pczt_payload} />
        <div style={{ fontSize: 14, opacity: 0.8, textAlign: 'center', maxWidth: 420 }}>
          Scan this animated QR with your Keystone to sign the transaction.
        </div>
      </div>

      <SigningVerify summary={summary} />

      <div style={{ display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
        <button
          type="button"
          onClick={() => {
            const bytes = decodeBase64ToBytes(signingRequest.pczt_payload);
            const blob = new Blob([bytes], { type: 'application/octet-stream' });
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = 'zkore-unsigned.pczt';
            a.click();
            URL.revokeObjectURL(url);
          }}
        >
          Export unsigned PCZT
        </button>

        <button type="button" onClick={() => setScannerOpen((v) => !v)}>
          {scannerOpen ? 'Close scanner' : 'Scan signed QR'}
        </button>
      </div>

      {scannerOpen ? (
        <QRScanner
          onScanned={(payloadBase64) => {
            setSignedPayload(payloadBase64);
            setScannerOpen(false);
            setError(null);
          }}
        />
      ) : null}

      <FileImport
        onImported={(payloadBase64) => {
          setSignedPayload(payloadBase64);
          setScannerOpen(false);
          setError(null);
        }}
      />

      <label style={{ display: 'grid', gap: 4, maxWidth: 420, width: '100%' }}>
        <span>Password</span>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.currentTarget.value)}
          disabled={submitting}
        />
      </label>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
        <button
          type="button"
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
        >
          {submitting ? 'Broadcasting…' : 'Broadcast transaction'}
        </button>
        <button
          type="button"
          onClick={() => {
            clearSensitive();
            navigate('/send');
          }}
          disabled={submitting}
        >
          Back
        </button>
      </div>
    </div>
  );
}
