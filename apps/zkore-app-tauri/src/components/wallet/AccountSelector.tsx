import type * as IPC from '../../types/ipc';

export function AccountSelector(props: {
  accounts: IPC.AccountInfo[];
  activeAccountId: number | null;
  onChange: (accountId: number) => void;
}) {
  const { accounts, activeAccountId, onChange } = props;

  if (accounts.length === 0) return null;

  return (
    <label style={{ display: 'inline-flex', gap: 8, alignItems: 'center' }}>
      <span>Account</span>
      <select
        value={activeAccountId ?? undefined}
        onChange={(e) => onChange(Number.parseInt(e.currentTarget.value, 10))}
      >
        {accounts.map((account) => {
          const suffix = account.account_type === 'HardwareSigner' ? ' (watch-only)' : '';
          return (
            <option key={account.id} value={account.id}>
              {account.name}
              {suffix}
            </option>
          );
        })}
      </select>
    </label>
  );
}

