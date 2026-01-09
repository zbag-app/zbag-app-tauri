import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Shield, Key, ArrowRight, Clock } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';

type Step = 'choice' | 'seed';

export function OnboardingBackup(props: {
  seedPhrase: string[];
  onCleared: () => void;
}) {
  const { seedPhrase, onCleared } = props;
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>('choice');

  const handleSkip = () => {
    onCleared();
    navigate('/');
  };

  const handleContinueToVerification = () => {
    // Keep seed phrase in memory for potential re-display if user comes back
    navigate('/backup');
  };

  // No seed phrase available - redirect to home
  if (seedPhrase.length !== 24) {
    return (
      <div className="flex min-h-screen items-center justify-center p-4">
        <Card className="w-full max-w-md animate-[scale-in_0.3s_ease-out]">
          <CardContent className="pt-6 text-center">
            <p className="text-muted-foreground">No seed phrase available.</p>
            <Button variant="outline" onClick={() => navigate('/')} className="mt-4">
              Go home
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  // Step 1: Choice screen
  if (step === 'choice') {
    return (
      <div className="flex min-h-screen items-center justify-center p-4">
        <Card className="w-full max-w-lg animate-[scale-in_0.3s_ease-out]">
          <CardHeader className="text-center">
            <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10">
              <Shield className="h-8 w-8 text-primary" />
            </div>
            <CardTitle className="font-display text-2xl">Wallet Created</CardTitle>
            <p className="text-sm text-muted-foreground mt-2">
              Your new wallet is ready. Now let's secure it with a backup.
            </p>
          </CardHeader>
          <CardContent className="space-y-4">
            <Card className="border-warning/50 bg-warning/5">
              <CardContent className="pt-4 pb-4">
                <p className="text-sm text-muted-foreground">
                  Your seed phrase is the only way to recover your wallet if you lose access.
                  Write it down and store it securely offline.
                </p>
              </CardContent>
            </Card>

            <div className="space-y-3 pt-2">
              <Button onClick={() => setStep('seed')} className="w-full" size="lg">
                <Key className="h-4 w-4" />
                Backup now
                <span className="ml-1 text-xs opacity-70">(recommended)</span>
              </Button>

              <Button variant="outline" onClick={handleSkip} className="w-full" size="lg">
                <Clock className="h-4 w-4" />
                I'll do it later
              </Button>
            </div>

            <p className="text-xs text-muted-foreground text-center pt-2">
              You can always backup your seed phrase later from the home screen.
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  // Step 2: Show seed phrase
  return (
    <div className="flex min-h-screen items-center justify-center p-4">
      <Card className="w-full max-w-2xl animate-[scale-in_0.3s_ease-out]">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10">
            <Key className="h-8 w-8 text-primary" />
          </div>
          <CardTitle className="font-display text-2xl">Write Down Your Seed Phrase</CardTitle>
        </CardHeader>
        <CardContent className="space-y-6">
          <Card className="border-warning/50 bg-warning/5">
            <CardContent className="pt-4 pb-4">
              <p className="text-sm text-muted-foreground">
                This is the only way to recover your wallet. Do not screenshot, copy/paste, or
                store it in cloud notes. Write it down on paper and keep it safe.
              </p>
            </CardContent>
          </Card>

          <div className="grid grid-cols-3 gap-2 select-none">
            {seedPhrase.map((word, idx) => (
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
