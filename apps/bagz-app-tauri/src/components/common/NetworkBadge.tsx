import type * as IPC from '../../types/ipc';

export function NetworkBadge(props: { network: IPC.Network }) {
  const { network } = props;
  const className =
    network === 'Mainnet'
      ? 'text-xs px-2 py-0.5 rounded-full bg-success/20 border border-success/30 text-success'
      : 'text-xs px-2 py-0.5 rounded-full bg-warning/20 border border-warning/30 text-warning';

  return <span className={className}>{network}</span>;
}
