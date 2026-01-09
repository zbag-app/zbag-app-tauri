import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Key, ArrowLeft, ArrowRight, Clipboard, QrCode } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { createKeystoneWallet, loadWallet } from '../services/ipc';
import { UFVKScanner } from '../components/keystone/UFVKScanner';

type Step = 'ufvk' | 'details';
type ImportMode = 'paste' | 'scan';

export function KeystoneSetup(props: {
  onCreated: (args: { wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }) => void;
}) {
  const { onCreated } = props;
  const navigate = useNavigate();

  const [step, setStep] = useState<Step>('ufvk');
  const [importMode, setImportMode] = useState<ImportMode>('paste');
  const [ufvk, setUfvk] = useState('');
  const [name, setName] = useState('Keystone');
  const [network, setNetwork] = useState<IPC.Network>('Testnet');
  const [password, setPassword] = useState('');
  const [passwordConfirm, setPasswordConfirm] = useState('');
  const [rememberUnlock, setRememberUnlock] = useState(false);
  const [birthdayHeight, setBirthdayHeight] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isValidUfvk = (value: string) => {
    const trimmed = value.trim().toLowerCase();
    return trimmed.startsWith('uview') || trimmed.startsWith('uivk');
  };

  const canContinueUfvk = ufvk.trim() && isValidUfvk(ufvk);

  const canSubmit = () => {
    if (submitting) return false;
    if (!name.trim()) return false;
    if (!password) return false;
    if (password !== passwordConfirm) return false;
    return true;
  };

  const submit = async () => {
    setError(null);

    if (!name.trim()) {
      setError('Wallet name is required.');
      return;
    }
    if (!password) {
      setError('Password is required.');
      return;
    }
    if (password !== passwordConfirm) {
      setError('Passwords do not match.');
      return;
    }

    setSubmitting(true);
    try {
      const birthday = birthdayHeight.trim() ? parseInt(birthdayHeight.trim(), 10) : undefined;
      if (birthdayHeight.trim() && (isNaN(birthday!) || birthday! < 0)) {
        setError('Birthday height must be a positive number.');
        setSubmitting(false);
        return;
      }

      const created = await createKeystoneWallet({
        name: name.trim(),
        network,
        password,
        remember_unlock: rememberUnlock,
        ufvk: ufvk.trim(),
        birthday_height: birthday,
      });

      if ('err' in created) {
        setError(created.err.message);
        return;
      }

      const load = await loadWallet({ wallet_id: created.ok.wallet.id });
      if ('err' in load) {
        setError(load.err.message);
        return;
      }

      onCreated({
        wallet: created.ok.wallet,
        accounts: load.ok.accounts,
      });

      navigate('/');
    } finally {
      setSubmitting(false);
    }
  };

  if (step === 'ufvk') {
    return (
      <div className="flex min-h-screen items-center justify-center p-4">
        <Card className="w-full max-w-md animate-[scale-in_0.3s_ease-out]">
          <CardHeader className="text-center">
            <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10">
              <Key className="h-8 w-8 text-primary" />
            </div>
            <CardTitle className="font-display text-2xl">Connect Keystone</CardTitle>
            <p className="text-sm text-muted-foreground">
              Import your Unified Full Viewing Key (UFVK) from Keystone
            </p>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex gap-2">
              <Button
                type="button"
                variant={importMode === 'paste' ? 'default' : 'outline'}
                onClick={() => setImportMode('paste')}
                size="sm"
              >
                <Clipboard className="h-4 w-4" />
                Paste UFVK
              </Button>
              <Button
                type="button"
                variant={importMode === 'scan' ? 'default' : 'outline'}
                onClick={() => setImportMode('scan')}
                size="sm"
              >
                <QrCode className="h-4 w-4" />
                Scan QR
              </Button>
            </div>

            {importMode === 'paste' ? (
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
                {ufvk.trim() && !isValidUfvk(ufvk) && (
                  <p className="text-sm text-destructive">
                    UFVK should start with "uview" (mainnet) or "uivk" (testnet)
                  </p>
                )}
              </div>
            ) : (
              <UFVKScanner
                onScanned={(scannedUfvk) => {
                  setUfvk(scannedUfvk);
                  setImportMode('paste');
                }}
                onCancel={() => setImportMode('paste')}
              />
            )}

            <div className="flex gap-3">
              <Button variant="outline" onClick={() => navigate(-1)}>
                <ArrowLeft className="h-4 w-4" />
                Back
              </Button>
              <Button
                onClick={() => setStep('details')}
                disabled={!canContinueUfvk}
                className="flex-1"
              >
                Continue
                <ArrowRight className="h-4 w-4" />
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-4">
      <Card className="w-full max-w-md animate-[scale-in_0.3s_ease-out]">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10">
            <Key className="h-8 w-8 text-primary" />
          </div>
          <CardTitle className="font-display text-2xl">Wallet Details</CardTitle>
          <p className="text-sm text-muted-foreground">
            Set up your hardware wallet
          </p>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-4"
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
          >
            <div className="space-y-2">
              <Label htmlFor="network">Network</Label>
              <select
                id="network"
                value={network}
                onChange={(e) => setNetwork(e.currentTarget.value as IPC.Network)}
                className="flex h-9 w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="Mainnet">Mainnet</option>
                <option value="Testnet">Testnet</option>
              </select>
              <p className="text-xs text-muted-foreground">
                Must match your Keystone device network
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="name">Wallet name</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.currentTarget.value)}
                placeholder="Keystone"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="password">Password</Label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.currentTarget.value)}
                placeholder="Enter password"
              />
              <p className="text-xs text-muted-foreground">
                Used to encrypt your wallet database locally
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="passwordConfirm">Confirm password</Label>
              <Input
                id="passwordConfirm"
                type="password"
                value={passwordConfirm}
                onChange={(e) => setPasswordConfirm(e.currentTarget.value)}
                placeholder="Confirm password"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="birthdayHeight">Birthday height (optional)</Label>
              <Input
                id="birthdayHeight"
                type="number"
                value={birthdayHeight}
                onChange={(e) => setBirthdayHeight(e.currentTarget.value)}
                placeholder="e.g., 2000000"
              />
              <p className="text-xs text-muted-foreground">
                Block height when this wallet was created. Speeds up initial sync.
                Leave blank to scan from Sapling activation (slower but complete).
              </p>
            </div>

            <label className="flex items-center gap-3 cursor-pointer">
              <input
                type="checkbox"
                checked={rememberUnlock}
                onChange={(e) => setRememberUnlock(e.currentTarget.checked)}
                className="rounded border-border h-4 w-4 accent-primary"
              />
              <span className="text-sm text-muted-foreground">
                Remember unlock (stores unlock material in OS keychain)
              </span>
            </label>

            {error && (
              <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                {error}
              </div>
            )}

            <div className="flex gap-3">
              <Button type="button" variant="outline" onClick={() => setStep('ufvk')}>
                <ArrowLeft className="h-4 w-4" />
                Back
              </Button>
              <Button type="submit" disabled={!canSubmit()} className="flex-1">
                <Key className="h-4 w-4" />
                {submitting ? 'Creating...' : 'Create Wallet'}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
