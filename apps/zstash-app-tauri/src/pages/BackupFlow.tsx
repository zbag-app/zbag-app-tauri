import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Key, ArrowRight, Lock } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { reauthWallet, viewSeedPhrase } from '../services/ipc';

type Step = 'password' | 'seed';

export function BackupFlow(props: { walletId: string }) {
  const { walletId } = props;
  const navigate = useNavigate();

  const [step, setStep] = useState<Step>('password');
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [seedWords, setSeedWords] = useState<string[] | null>(null);

  const handlePasswordSubmit = async () => {
    setError(null);
    setLoading(true);
    try {
      const tokenRes = await reauthWallet({
        wallet_id: walletId,
        password,
        purpose: 'ViewSeedPhrase',
      });
      if ('err' in tokenRes) {
        setError(tokenRes.err.message);
        return;
      }

      const seedRes = await viewSeedPhrase({
        wallet_id: walletId,
        reauth_token: tokenRes.ok.reauth_token,
      });
      if ('err' in seedRes) {
        setError(seedRes.err.message);
        return;
      }

      setSeedWords(seedRes.ok.seed_phrase);
      setStep('seed');
    } finally {
      setLoading(false);
    }
  };

  const handleContinueToVerification = () => {
    // Clear seed from local state for security
    setSeedWords(null);
    navigate('/backup');
  };

  // Step 1: Password prompt
  if (step === 'password') {
    return (
      <div className="flex min-h-screen items-center justify-center p-4 grid-bg">
        <Card className="w-full max-w-md animate-[scale-in_0.3s_ease-out]">
          <CardHeader className="text-center">
            <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-none bg-primary/10">
              <Lock className="h-8 w-8 text-primary" />
            </div>
            <CardTitle className="font-display text-2xl">Backup Your Wallet</CardTitle>
            <p className="text-sm text-muted-foreground mt-2">
              Enter your password to view your seed phrase.
            </p>
          </CardHeader>
          <CardContent>
            <form
              className="space-y-4"
              onSubmit={(e) => {
                e.preventDefault();
                void handlePasswordSubmit();
              }}
            >
              <div className="space-y-2">
                <Label htmlFor="backup-password">Wallet password</Label>
                <Input
                  id="backup-password"
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.currentTarget.value)}
                  placeholder="Enter your password"
                  autoFocus
                />
              </div>

              {error && (
                <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                  {error}
                </div>
              )}

              <div className="flex gap-3">
                <Button type="submit" disabled={!password || loading} className="flex-1">
                  {loading ? 'Verifying...' : 'Continue'}
                </Button>
                <Button type="button" variant="outline" onClick={() => navigate('/')}>
                  Cancel
                </Button>
              </div>
            </form>
          </CardContent>
        </Card>
      </div>
    );
  }

  // Step 2: Show seed phrase
  if (!seedWords || seedWords.length !== 24) {
    return (
      <div className="flex min-h-screen items-center justify-center p-4 grid-bg">
        <Card className="w-full max-w-md animate-[scale-in_0.3s_ease-out]">
          <CardContent className="pt-6 text-center">
            <p className="text-muted-foreground">Unable to load seed phrase.</p>
            <Button variant="outline" onClick={() => navigate('/')} className="mt-4">
              Go home
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-4 grid-bg">
      <Card className="w-full max-w-2xl animate-[scale-in_0.3s_ease-out]">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-none bg-primary/10">
            <Key className="h-8 w-8 text-primary" />
          </div>
          <CardTitle className="font-display text-2xl">Your Seed Phrase</CardTitle>
        </CardHeader>
        <CardContent className="space-y-6">
          <Card className="border-warning/50 bg-warning/5">
            <CardContent className="pt-4 pb-4">
              <p className="text-sm text-muted-foreground">
                Write down these 24 words in order. Do not screenshot, copy/paste, or store in
                cloud notes. Keep this backup safe and offline.
              </p>
            </CardContent>
          </Card>

          <div className="grid grid-cols-3 gap-2 select-none">
            {seedWords.map((word, idx) => (
              <div
                key={idx}
                className="flex items-center gap-2 rounded-lg border border-border bg-muted/50 px-3 py-2"
              >
                <span className="w-6 text-sm text-muted-foreground">{idx + 1}.</span>
                <span className="font-mono font-semibold">{word}</span>
              </div>
            ))}
          </div>

          <Button onClick={handleContinueToVerification} className="w-full" size="lg">
            I've written it down
            <ArrowRight className="h-4 w-4" />
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
