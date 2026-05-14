import { useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { RotateCcw, Shield } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';

export type RestoreFlowData = {
  name: string;
  network: IPC.Network;
  password: string;
  remember_unlock: boolean;
  seed_phrase: string;
};

function countWords(value: string): number {
  return value
    .trim()
    .split(/\s+/)
    .map((w) => w.trim())
    .filter(Boolean).length;
}

export function RestoreWallet() {
  const navigate = useNavigate();

  const [name, setName] = useState('');
  const [network, setNetwork] = useState<IPC.Network>('Testnet');
  const [password, setPassword] = useState('');
  const [passwordConfirm, setPasswordConfirm] = useState('');
  const [seedPhrase, setSeedPhrase] = useState('');
  const [error, setError] = useState<string | null>(null);

  const seedWordCount = useMemo(() => countWords(seedPhrase), [seedPhrase]);

  const submit = () => {
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
    if (seedWordCount !== 24) {
      setError('Seed phrase must be exactly 24 words.');
      return;
    }

    const flowData: RestoreFlowData = {
      name: name.trim(),
      network,
      password,
      remember_unlock: false,
      seed_phrase: seedPhrase.trim(),
    };

    // Clear sensitive inputs before navigating
    setSeedPhrase('');
    setPassword('');
    setPasswordConfirm('');

    // Pass flow data via navigation state (next page replaces the history entry to clear it).
    navigate('/restore/birthday', { state: flowData });
  };

  return (
    <div className="flex min-h-screen items-center justify-center p-4">
      <Card className="w-full max-w-lg animate-[scale-in_0.3s_ease-out]">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-none bg-primary/10">
            <Shield className="h-8 w-8 text-primary" />
          </div>
          <CardTitle className="font-display text-2xl">Restore Wallet</CardTitle>
          <p className="text-sm text-muted-foreground">
            Recover your wallet from a seed phrase
          </p>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-4"
            onSubmit={(e) => {
              e.preventDefault();
              submit();
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
                placeholder="My Restored Wallet"
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

            <div className="space-y-2">
              <Label htmlFor="seedPhrase">Seed phrase (24 words)</Label>
              <textarea
                id="seedPhrase"
                value={seedPhrase}
                onChange={(e) => setSeedPhrase(e.currentTarget.value)}
                rows={4}
                placeholder="Enter your 24-word seed phrase..."
                className="flex w-full rounded-none border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring font-mono"
              />
              <p className="text-xs text-muted-foreground">{seedWordCount} / 24 words</p>
            </div>

            {error && (
              <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                {error}
              </div>
            )}

            <Button type="submit" className="w-full">
              <RotateCcw className="h-4 w-4" />
              Continue
            </Button>

            <div className="text-center">
              <Link to="/" className="text-sm text-primary hover:underline">
                Create new wallet instead
              </Link>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
