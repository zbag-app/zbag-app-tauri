import type * as IPC from '../../types/ipc';
import { useRef } from 'react';
import { X } from 'lucide-react';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { Card, CardContent, CardHeader, CardTitle } from '../ui/card';
import { Button } from '../ui/button';

export function TorErrorDialog(props: {
  state: IPC.TorState;
  onRetry: () => void;
  onDisable: () => void;
  onClose: () => void;
}) {
  const { state, onRetry, onDisable, onClose } = props;
  const dialogRef = useRef<HTMLDivElement>(null);

  useFocusTrap(dialogRef, true);
  useKeyboardShortcuts('esc', onClose);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Tor error"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
    >
      <Card ref={dialogRef} className="w-full max-w-xl animate-[fade-in-up_0.2s_ease-out]">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-4">
          <CardTitle className="text-lg">Tor error</CardTitle>
          <Button
            variant="ghost"
            size="sm"
            onClick={onClose}
            aria-label="Close Tor error dialog"
            className="h-8 w-8 p-0"
          >
            <X className="h-4 w-4" />
          </Button>
        </CardHeader>

        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Tor is enabled but currently in an error state. Network requests will fail closed until Tor is healthy again.
          </p>

          <div className="rounded-none border border-destructive/30 bg-destructive/10 p-3">
            <div className="font-semibold text-sm mb-1">Last error</div>
            <div className="text-sm whitespace-pre-wrap text-destructive">{state.last_error ?? 'Unknown error'}</div>
          </div>

          <div className="flex gap-2 justify-end">
            <Button variant="outline" onClick={onDisable}>
              Disable Tor
            </Button>
            <Button onClick={onRetry}>
              Retry
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
