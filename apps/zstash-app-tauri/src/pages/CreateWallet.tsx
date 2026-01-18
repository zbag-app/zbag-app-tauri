import { useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { Key, Plus, Shield } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { createWallet, loadWallet } from '../services/ipc';

export function CreateWallet(props: {
  onCreated: (args: {
    wallet: IPC.WalletInfo;
    accounts: IPC.AccountInfo[];
    seedPhrase: string[];
  }) => void;
}) {
  const { onCreated } = props;
  const navigate = useNavigate();

  const [name, setName] = useState('');
  const [network, setNetwork] = useState<IPC.Network>('Testnet');
  const [password, setPassword] = useState('');
  const [passwordConfirm, setPasswordConfirm] = useState('');
  const [rememberUnlock, setRememberUnlock] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      const created = await createWallet({
        name: name.trim(),
        network,
        password,
        remember_unlock: rememberUnlock,
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
        seedPhrase: created.ok.seed_phrase,
      });

      navigate('/onboarding-backup');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="flex min-h-screen items-center justify-center p-4">
      <Card className="w-full max-w-md animate-[scale-in_0.3s_ease-out]">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-none bg-primary/10">
            <Shield className="h-8 w-8 text-primary" />
          </div>
          <CardTitle className="font-display text-2xl">Create Wallet</CardTitle>
          <p className="text-sm text-muted-foreground">
            Set up a new Zcash wallet
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
                className="flex h-9 w-full rounded-none border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="Mainnet">Mainnet</option>
                <option value="Testnet">Testnet</option>
              </select>
            </div>

            <div className="space-y-2">
              <Label htmlFor="name">Wallet name</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.currentTarget.value)}
                placeholder="My Wallet"
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

            <label className="flex items-center gap-3 cursor-pointer">
              <input
                type="checkbox"
                checked={rememberUnlock}
                onChange={(e) => setRememberUnlock(e.currentTarget.checked)}
                className="rounded-none border-border h-4 w-4 accent-primary"
              />
              <span className="text-sm text-muted-foreground">
                Remember unlock (stores unlock material in OS keychain)
              </span>
            </label>

            {error && (
              <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                {error}
              </div>
            )}

            <Button type="submit" disabled={submitting} className="w-full">
              <Plus className="h-4 w-4" />
              {submitting ? 'Creating...' : 'Create wallet'}
            </Button>

            <div className="text-center space-y-2">
              <Link to="/restore" className="text-sm text-primary hover:underline block">
                Restore from seed phrase
              </Link>
              <Link to="/keystone/setup" className="text-sm text-muted-foreground hover:text-foreground flex items-center justify-center gap-1 transition-colors">
                <Key className="h-3 w-3" />
                Connect hardware wallet
              </Link>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
