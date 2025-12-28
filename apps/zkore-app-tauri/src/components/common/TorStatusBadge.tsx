import type * as IPC from '../../types/ipc';

const COLORS: Record<IPC.TorStatus, string> = {
  Off: '#777',
  Connecting: '#b45309',
  On: '#15803d',
  Error: '#b91c1c',
};

export function TorStatusBadge(props: { state: IPC.TorState | null }) {
  const state = props.state;
  const status: IPC.TorStatus = state?.status ?? 'Off';
  const enabled = state?.enabled ?? false;

  const label = enabled ? `Tor: ${status}` : 'Tor: Off';

  return (
    <span
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 8,
        padding: '4px 10px',
        borderRadius: 999,
        border: '1px solid #ddd',
        background: '#fff',
        fontSize: 12,
      }}
      aria-label={label}
      title={state?.last_error ? `Tor error: ${state.last_error}` : label}
    >
      <span
        aria-hidden="true"
        style={{
          width: 8,
          height: 8,
          borderRadius: 999,
          background: enabled ? COLORS[status] : COLORS.Off,
          display: 'inline-block',
        }}
      />
      <span>{label}</span>
      <span style={{ opacity: 0.7 }}>(beta)</span>
    </span>
  );
}

