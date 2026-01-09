import type * as IPC from '../../types/ipc';

const STATUS_DOT_CLASSES: Record<IPC.TorStatus, string> = {
  Off: 'bg-muted-foreground',
  Connecting: 'bg-warning',
  On: 'bg-success',
  Error: 'bg-destructive',
};

export function TorStatusBadge(props: { state: IPC.TorState | null }) {
  const state = props.state;
  const status: IPC.TorStatus = state?.status ?? 'Off';
  const enabled = state?.enabled ?? false;

  const label = enabled ? `Tor: ${status}` : 'Tor: Off';
  const dotClass = enabled ? STATUS_DOT_CLASSES[status] : STATUS_DOT_CLASSES.Off;

  return (
    <span
      className="inline-flex items-center gap-2 px-2.5 py-1 rounded-full border border-border bg-card text-xs"
      aria-label={label}
      title={state?.last_error ? `Tor error: ${state.last_error}` : label}
    >
      <span
        aria-hidden="true"
        className={`w-2 h-2 rounded-full ${dotClass}`}
      />
      <span>{label}</span>
      <span className="text-muted-foreground">(beta)</span>
    </span>
  );
}
