import type * as IPC from '../../types/ipc';

export function TorErrorDialog(props: {
  state: IPC.TorState;
  onRetry: () => void;
  onDisable: () => void;
  onClose: () => void;
}) {
  const { state, onRetry, onDisable, onClose } = props;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Tor error"
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.45)',
        display: 'grid',
        placeItems: 'center',
        padding: 16,
        zIndex: 50,
      }}
    >
      <div style={{ background: 'white', borderRadius: 12, padding: 16, maxWidth: 560, width: '100%' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
          <h2 style={{ margin: 0 }}>Tor error</h2>
          <button type="button" onClick={onClose} aria-label="Close Tor error dialog">
            Close
          </button>
        </div>

        <div style={{ marginTop: 12, display: 'grid', gap: 10 }}>
          <div style={{ fontSize: 14, opacity: 0.85 }}>
            Tor is enabled but currently in an error state. Network requests will fail closed until Tor is healthy again.
          </div>
          <div style={{ fontSize: 13, background: '#fff5f5', border: '1px solid #fecaca', padding: 10, borderRadius: 8 }}>
            <div style={{ fontWeight: 600, marginBottom: 4 }}>Last error</div>
            <div style={{ whiteSpace: 'pre-wrap' }}>{state.last_error ?? 'Unknown error'}</div>
          </div>

          <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
            <button type="button" onClick={onDisable}>
              Disable Tor
            </button>
            <button type="button" onClick={onRetry}>
              Retry
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

