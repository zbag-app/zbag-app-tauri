import { useState } from 'react';
import { LogoutDialogModal } from './LogoutDialogModal';
import { Button } from '../ui/button';

interface LogoutDialogProps {
  walletId: string;
  triggerLabel: string;
  onLogout: () => void;
}

export function LogoutDialog(props: LogoutDialogProps) {
  const { walletId, triggerLabel, onLogout } = props;

  const [open, setOpen] = useState(false);

  return (
    <>
      <Button variant="outline" size="sm" onClick={() => setOpen(true)}>
        {triggerLabel}
      </Button>

      <LogoutDialogModal
        walletId={walletId}
        open={open}
        onClose={() => setOpen(false)}
        onLogout={onLogout}
      />
    </>
  );
}
