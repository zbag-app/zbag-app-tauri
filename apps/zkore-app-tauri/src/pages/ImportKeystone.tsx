import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Key, ArrowLeft, Clipboard, QrCode } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { importUfvk, loadWallet } from '../services/ipc';
import { UFVKScanner } from '../components/keystone/UFVKScanner';

type ImportMode = 'paste' | 'scan';

export function ImportKeystone(props: {
  walletId: string;
  onAccountsUpdated: (accounts: IPC.AccountInfo[]) => void;
}) {
  const { walletId, onAccountsUpdated } = props;
  const navigate = useNavigate();

  const [mode, setMode] = useState<ImportMode>('paste');
  const [name, setName] = useState('Keystone');
  const [ufvk, setUfvk] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const canSubmit = useMemo(() => {
    if (submitting) return false;
    if (!name.trim()) return false;
    if (!ufvk.trim()) return false;
    return true;
  }, [name, ufvk, submitting]);

  const submit = async () => {
    setSubmitting(true);
    setError(null);

    const res = await importUfvk({ wallet_id: walletId, ufvk, name });
    if ('err' in res) {
      setSubmitting(false);
      setError(res.err.message);
      return;
    }

    const reloaded = await loadWallet({ wallet_id: walletId });
    setSubmitting(false);
    if ('err' in reloaded) {
      setError(reloaded.err.message);
      return;
    }
    if (reloaded.ok.lock_status === 'Locked') {
      setError('Wallet is locked. Unlock and try again.');
      return;
    }

    onAccountsUpdated(reloaded.ok.accounts);
    navigate('/');
  };

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <Key className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Import Keystone</h1>
      </div>

      <Card>
        <CardContent className="pt-6">
          <p className="text-sm text-muted-foreground">
            Import a Unified Full Viewing Key (UFVK) to create a watch-only account. Spending from this
            account requires Keystone signing.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Account Details</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="accountName">Account name</Label>
            <Input
              id="accountName"
              value={name}
              onChange={(e) => setName(e.currentTarget.value)}
              placeholder="Keystone"
            />
          </div>

          <div className="flex gap-2">
            <Button
              type="button"
              variant={mode === 'paste' ? 'default' : 'outline'}
              onClick={() => setMode('paste')}
              size="sm"
            >
              <Clipboard className="h-4 w-4" />
              Paste UFVK
            </Button>
            <Button
              type="button"
              variant={mode === 'scan' ? 'default' : 'outline'}
              onClick={() => setMode('scan')}
              size="sm"
            >
              <QrCode className="h-4 w-4" />
              Scan QR
            </Button>
          </div>

          {mode === 'paste' ? (
            <div className="space-y-2">
              <Label htmlFor="ufvk">UFVK</Label>
              <textarea
                id="ufvk"
                value={ufvk}
                onChange={(e) => setUfvk(e.currentTarget.value)}
                rows={4}
                placeholder="uview..."
                className="flex w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring font-mono"
              />
            </div>
          ) : (
            <UFVKScanner
              onScanned={(scannedUfvk) => {
                setUfvk(scannedUfvk);
                setMode('paste');
              }}
              onCancel={() => setMode('paste')}
            />
          )}

          {error && (
            <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}

          <div className="flex gap-3">
            <Button variant="outline" onClick={() => navigate(-1)} disabled={submitting}>
              <ArrowLeft className="h-4 w-4" />
              Back
            </Button>
            <Button onClick={submit} disabled={!canSubmit} className="flex-1">
              {submitting ? 'Importing...' : 'Import'}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
