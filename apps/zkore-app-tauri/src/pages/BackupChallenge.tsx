import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import * as IPC from '../types/ipc';
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
      <div style={{ display: 'grid', gap: 12, padding: 16 }}>
        <h1>Backup verification</h1>
        <div>{loading ? 'Loading…' : 'No challenge.'}</div>
        {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}
        <button type="button" onClick={refresh} disabled={loading}>
          Get challenge
        </button>
      </div>
    );
  }

  return (
    <form
      style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 520 }}
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <h1>Backup verification</h1>
      <p>Enter the requested words from your 24-word seed phrase.</p>

      <div style={{ display: 'grid', gap: 10 }}>
        {sortedIndices.map((index) => (
          <label key={index} style={{ display: 'grid', gap: 4 }}>
            <span>Word #{index}</span>
            <input
              value={inputs[index] ?? ''}
              onChange={(e) =>
                setInputs((prev) => ({ ...prev, [index]: e.currentTarget.value }))
              }
            />
          </label>
        ))}
      </div>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'flex', gap: 8 }}>
        <button type="submit" disabled={loading}>
          Verify
        </button>
        <button type="button" onClick={refresh} disabled={loading}>
          New challenge
        </button>
        <button type="button" onClick={() => navigate('/')}>
          Cancel
        </button>
      </div>
    </form>
  );
}
