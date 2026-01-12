import { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import { reauthWallet, logoutWallet, stopSync } from '../../services/ipc';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { Card, CardContent, CardHeader, CardTitle } from '../ui/card';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Label } from '../ui/label';

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
    <>
      <Button variant="outline" size="sm" onClick={() => setOpen(true)}>
        {triggerLabel}
      </Button>

      {open && (
        <div
          role="dialog"
          aria-modal="true"
          onClick={(e) => {
            if (e.target === e.currentTarget) setOpen(false);
          }}
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
        >
          <Card ref={dialogRef} className="w-full max-w-md animate-[fade-in-up_0.2s_ease-out]">
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-4">
              <CardTitle className="text-lg">Logout</CardTitle>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setOpen(false)}
                aria-label="Close logout dialog"
                className="h-8 w-8 p-0"
              >
                <X className="h-4 w-4" />
              </Button>
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
                  <Label htmlFor="logout-password">Wallet password</Label>
                  <Input
                    id="logout-password"
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.currentTarget.value)}
                    autoFocus
                  />
                </div>
                {error && (
                  <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                    {error}
                  </div>
                )}
                <Button type="submit" disabled={!password || loading} className="w-full">
                  {loading ? 'Logging out...' : 'Logout'}
                </Button>
              </form>
            </CardContent>
          </Card>
        </div>
      )}
    </>
  );
}
