import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ShieldCheck, RefreshCw } from 'lucide-react';
import * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { getBackupChallenge, verifyBackup } from '../services/ipc';

type ChallengeState = {
  challengeId: string;
  indices: number[];
  expiresAt: number;
};

export function BackupChallenge(props: { walletId: string; onVerified: () => void }) {
  const { walletId, onVerified } = props;
  const navigate = useNavigate();

  const [challenge, setChallenge] = useState<ChallengeState | null>(null);
  const [inputs, setInputs] = useState<Record<number, string>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const sortedIndices = useMemo(
    () => (challenge ? challenge.indices.slice().sort((a, b) => a - b) : []),
    [challenge]
  );

  const refresh = async () => {
    setError(null);
    setLoading(true);
    try {
      const res = await getBackupChallenge({ wallet_id: walletId });
      if ('err' in res) {
        setError(res.err.message);
        return;
      }

      const c = res.ok.challenge;
      setChallenge({ challengeId: c.challenge_id, indices: c.indices, expiresAt: c.expires_at });
      setInputs({});
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    return () => {
      setChallenge(null);
      setInputs({});
      setError(null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [walletId]);

  const submit = async () => {
    if (!challenge) return;
    setError(null);
    setLoading(true);
    try {
      const word_challenges: Record<number, string> = {};
      for (const index of challenge.indices) {
        word_challenges[index] = inputs[index] ?? '';
      }

      const res = await verifyBackup({
        wallet_id: walletId,
        challenge_id: challenge.challengeId,
        word_challenges,
      });

      if ('err' in res) {
        const code = res.err.code as string;
        const shouldRefresh =
          code === IPC.ErrorCodes.BACKUP_CHALLENGE_INVALID ||
          code === IPC.ErrorCodes.BACKUP_CHALLENGE_EXPIRED ||
          code === IPC.ErrorCodes.BACKUP_CHALLENGE_TOO_MANY_ATTEMPTS;
        setError(res.err.message);
        if (shouldRefresh) {
          await refresh();
        }
        return;
      }

      onVerified();
      navigate('/');
    } finally {
      setLoading(false);
    }
  };

  if (!challenge) {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <ShieldCheck className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Backup Verification</h1>
        </div>

        <Card>
          <CardContent className="pt-6">
            <p className="text-muted-foreground">
              {loading ? 'Loading...' : 'No challenge.'}
            </p>
            {error && (
              <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive mt-4">
                {error}
              </div>
            )}
            <Button onClick={refresh} disabled={loading} className="mt-4">
              <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
              Get challenge
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <ShieldCheck className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Backup Verification</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Verify Your Seed Phrase</CardTitle>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-4"
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
          >
            <p className="text-sm text-muted-foreground">
              Enter the requested words from your 24-word seed phrase.
            </p>

            <div className="space-y-3">
              {sortedIndices.map((index) => (
                <div key={index} className="space-y-2">
                  <Label htmlFor={`word-${index}`}>Word #{index}</Label>
                  <Input
                    id={`word-${index}`}
                    value={inputs[index] ?? ''}
                    onChange={(e) => {
                      const value = e.currentTarget.value;
                      setInputs((prev) => ({ ...prev, [index]: value }));
                    }}
                    placeholder={`Enter word #${index}`}
                  />
                </div>
              ))}
            </div>

            {error && (
              <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                {error}
              </div>
            )}

            <div className="flex gap-3">
              <Button type="submit" disabled={loading} className="flex-1">
                {loading ? 'Verifying...' : 'Verify'}
              </Button>
              <Button type="button" variant="outline" onClick={refresh} disabled={loading}>
                <RefreshCw className="h-4 w-4" />
                New challenge
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
