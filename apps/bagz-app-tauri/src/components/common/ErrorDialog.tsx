import { useRef } from 'react';
import { X } from 'lucide-react';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { Card, CardContent, CardHeader, CardTitle } from '../ui/card';
import { Button } from '../ui/button';

export type ErrorDialogAction = {
  label: string;
  onClick: () => void;
};

export type ErrorDialogError = {
  code: string;
  message: string;
};

export function ErrorDialog(props: {
  title: string;
  error: ErrorDialogError;
  primaryAction: ErrorDialogAction;
  secondaryAction?: ErrorDialogAction;
}) {
  const { title, error, primaryAction, secondaryAction } = props;
  const dialogRef = useRef<HTMLDivElement>(null);

  useFocusTrap(dialogRef, true);
  useKeyboardShortcuts('esc', primaryAction.onClick);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      onClick={(e) => {
        if (e.target === e.currentTarget) primaryAction.onClick();
      }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4"
    >
      <Card ref={dialogRef} className="w-full max-w-xl animate-[fade-in-up_0.2s_ease-out]">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-4">
          <CardTitle className="text-lg">{title}</CardTitle>
          <Button
            variant="ghost"
            size="sm"
            onClick={primaryAction.onClick}
            aria-label="Close error dialog"
            className="h-8 w-8 p-0"
          >
            <X className="h-4 w-4" />
          </Button>
        </CardHeader>

        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            If you contact support, share this error code: <code className="text-foreground">{error.code}</code>
          </p>

          <div className="rounded-none border border-destructive/30 bg-destructive/10 p-3">
            <div className="font-semibold text-sm mb-1">Message</div>
            <div className="text-sm whitespace-pre-wrap text-destructive">{error.message}</div>
          </div>

          <div className="flex gap-2 justify-end flex-wrap">
            {secondaryAction ? (
              <Button variant="outline" onClick={secondaryAction.onClick}>
                {secondaryAction.label}
              </Button>
            ) : null}
            <Button onClick={primaryAction.onClick}>
              {primaryAction.label}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
