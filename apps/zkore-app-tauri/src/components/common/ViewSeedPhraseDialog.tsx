import { useEffect, useRef, useState } from 'react';
import { reauthWallet, viewSeedPhrase } from '../../services/ipc';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';

export function ViewSeedPhraseDialog(props: { walletId: string; triggerLabel: string }) {
  const { walletId, triggerLabel } = props;

  const [open, setOpen] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [seedWords, setSeedWords] = useState<string[] | null>(null);

  useFocusTrap(dialogRef, open);
  useKeyboardShortcuts('esc', () => setOpen(false), open);

  useEffect(() => {
    if (!open) {
      setPassword('');
      setError(null);
      setSeedWords(null);
    }
  }, [open]);

  const submit = async () => {
    setError(null);
    setLoading(true);
    try {
      const tokenRes = await reauthWallet({ wallet_id: walletId, password, purpose: 'ViewSeedPhrase' });
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
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ display: 'inline-flex', gap: 8, alignItems: 'center' }}>
      <button type="button" onClick={() => setOpen(true)}>
        {triggerLabel}
      </button>

      {open ? (
        <div
          role="dialog"
          aria-modal="true"
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.45)',
            display: 'grid',
            placeItems: 'center',
            padding: 16,
          }}
        >
          <div
            ref={dialogRef}
            style={{ background: 'white', borderRadius: 12, padding: 16, maxWidth: 720, width: '100%' }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
              <h2 style={{ margin: 0 }}>Seed phrase</h2>
              <button type="button" onClick={() => setOpen(false)} aria-label="Close seed phrase dialog">
                Close
              </button>
            </div>

            {seedWords ? (
              <div style={{ display: 'grid', gap: 12, marginTop: 12 }}>
                <div style={{ fontSize: 14, opacity: 0.85 }}>
                  Keep this private. Do not copy/paste or screenshot.
                </div>
                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
                    gap: 8,
                    userSelect: 'none',
                  }}
                >
                  {seedWords.map((word, idx) => (
                    <div
                      key={idx}
                      style={{
                        display: 'flex',
                        gap: 8,
                        padding: 8,
                        border: '1px solid #ddd',
                        borderRadius: 8,
                        background: '#fafafa',
                      }}
                    >
                      <span style={{ width: 28, opacity: 0.7 }}>{idx + 1}.</span>
                      <strong>{word}</strong>
                    </div>
                  ))}
                </div>
              </div>
            ) : (
              <form
                style={{ display: 'grid', gap: 12, marginTop: 12 }}
                onSubmit={(e) => {
                  e.preventDefault();
                  void submit();
                }}
              >
                <label style={{ display: 'grid', gap: 4 }}>
                  <span>Wallet password</span>
                  <input
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.currentTarget.value)}
                  />
                </label>
                {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}
                <button type="submit" disabled={!password || loading}>
                  {loading ? 'Verifying…' : 'View seed phrase'}
                </button>
              </form>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
