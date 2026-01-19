import { useEffect, useRef, useState } from 'react';
import { X } from 'lucide-react';
import { logoutWallet, stopSync } from '../../services/ipc';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { Card, CardContent, CardHeader, CardTitle } from '../ui/card';
import { Button } from '../ui/button';

interface LogoutConfirmDialogProps {
  walletId: string | null;
  open: boolean;
  onClose: () => void;
  onLogout: () => void;
}

export function LogoutConfirmDialog(props: LogoutConfirmDialogProps) {
  const { walletId, open, onClose, onLogout } = props;

  const dialogRef = useRef<HTMLDivElement>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useFocusTrap(dialogRef, open);
  useKeyboardShortcuts(
    'esc',
    () => {
      if (!loading) onClose();
    },
    open
  );

  useEffect(() => {
    if (!open) {
      setLoading(false);
      setError(null);
    }
  }, [open]);

  if (!open) return null;

  const submit = async () => {
    if (!walletId) return;

    setError(null);
    setLoading(true);
    try {
      // Best effort stop sync - do not block logout on failures.
      try {
        const stopRes = await stopSync({ wallet_id: walletId });
        if ('err' in stopRes) {
          console.warn(
            'Failed to stop sync before logout',
            stopRes.err.code,
            stopRes.err.message
          );
        }
      } catch (err) {
        console.warn('Failed to stop sync before logout', err);
      }

      const logoutRes = await logoutWallet({ wallet_id: walletId });
      if ('err' in logoutRes) {
        setError(logoutRes.err.message);
        return;
      }

      onClose();
      onLogout();
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      role="dialog"
      aria-modal="true"
      onClick={(e) => {
        if (e.target === e.currentTarget && !loading) onClose();
      }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
    >
      <Card ref={dialogRef} className="w-full max-w-md animate-[fade-in-up_0.2s_ease-out]">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-4">
          <CardTitle className="text-lg">Logout</CardTitle>
          <Button
            variant="ghost"
            size="sm"
            onClick={onClose}
            aria-label="Close logout dialog"
            className="h-8 w-8 p-0"
            disabled={loading}
          >
            <X className="h-4 w-4" />
          </Button>
        </CardHeader>

        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Are you sure you want to logout? You will need to unlock your wallet again to access
            it.
          </p>
          {error && (
            <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}
          <div className="flex gap-2 justify-end">
            <Button variant="outline" onClick={onClose} disabled={loading}>
              Cancel
            </Button>
            <Button onClick={() => void submit()} disabled={loading || !walletId}>
              {loading ? 'Logging out...' : 'Logout'}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
