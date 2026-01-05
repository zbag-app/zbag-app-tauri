import { useEffect, useRef, useState } from 'react';
import { reauthWallet, logoutWallet, stopSync } from '../../services/ipc';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';

interface LogoutDialogProps {
  walletId: string;
  triggerLabel: string;
  onLogout: () => void;
}

export function LogoutDialog(props: LogoutDialogProps) {
  const { walletId, triggerLabel, onLogout } = props;

  const [open, setOpen] = useState(false);
  const dialogRef = useRef<HTMLDivElement>(null);
  const [password, setPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useFocusTrap(dialogRef, open);
  useKeyboardShortcuts('esc', () => setOpen(false), open);

  useEffect(() => {
    if (!open) {
      setPassword('');
      setError(null);
    }
  }, [open]);

  const submit = async () => {
    setError(null);
    setLoading(true);
    try {
      const tokenRes = await reauthWallet({ wallet_id: walletId, password, purpose: 'Logout' });
      if ('err' in tokenRes) {
        setError(tokenRes.err.message);
        return;
      }

      // Best effort stop sync - ignore errors
      await stopSync({ wallet_id: walletId }).catch(() => {});

      const logoutRes = await logoutWallet({
        wallet_id: walletId,
        reauth_token: tokenRes.ok.reauth_token,
      });
      if ('err' in logoutRes) {
        setError(logoutRes.err.message);
        return;
      }

      setOpen(false);
      onLogout();
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
            style={{ background: 'white', borderRadius: 12, padding: 16, maxWidth: 400, width: '100%' }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
              <h2 style={{ margin: 0 }}>Logout</h2>
              <button type="button" onClick={() => setOpen(false)} aria-label="Close logout dialog">
                Close
              </button>
            </div>

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
                {loading ? 'Logging out...' : 'Logout'}
              </button>
            </form>
          </div>
        </div>
      ) : null}
    </div>
  );
}
