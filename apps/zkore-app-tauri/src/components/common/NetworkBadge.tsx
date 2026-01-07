import type * as IPC from '../../types/ipc';

export function NetworkBadge(props: { network: IPC.Network }) {
  const { network } = props;
  const style =
    network === 'Mainnet'
      ? { background: '#dcfce7', border: '1px solid #16a34a', color: '#166534' }
      : { background: '#ffedd5', border: '1px solid #f97316', color: '#9a3412' };

  return (
    <span
      style={{
        fontSize: 12,
        padding: '2px 8px',
        borderRadius: 999,
        ...style,
      }}
    >
      {network}
    </span>
  );
}

