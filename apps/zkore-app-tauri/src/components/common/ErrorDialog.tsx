import { useRef } from 'react';
import { useFocusTrap } from '../../hooks/useFocusTrap';
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';

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
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.45)',
        display: 'grid',
        placeItems: 'center',
        padding: 16,
        zIndex: 100,
      }}
    >
      <div
        ref={dialogRef}
        style={{ background: 'white', borderRadius: 12, padding: 16, maxWidth: 560, width: '100%' }}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
          <h2 style={{ margin: 0 }}>{title}</h2>
          <button type="button" onClick={primaryAction.onClick} aria-label="Close error dialog">
            Close
          </button>
        </div>

        <div style={{ marginTop: 12, display: 'grid', gap: 10 }}>
          <div style={{ fontSize: 14, opacity: 0.85 }}>
            If you contact support, share this error code: <code>{error.code}</code>
          </div>
          <div style={{ fontSize: 13, background: '#fff5f5', border: '1px solid #fecaca', padding: 10, borderRadius: 8 }}>
            <div style={{ fontWeight: 600, marginBottom: 4 }}>Message</div>
            <div style={{ whiteSpace: 'pre-wrap' }}>{error.message}</div>
          </div>

          <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', flexWrap: 'wrap' }}>
            {secondaryAction ? (
              <button type="button" onClick={secondaryAction.onClick}>
                {secondaryAction.label}
              </button>
            ) : null}
            <button type="button" onClick={primaryAction.onClick}>
              {primaryAction.label}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

