import { useEffect, useMemo, useState } from 'react';
import type * as IPC from '../types/ipc';

const storageKey = (walletId: string) => `bagz.activeAccount.${walletId}`;

export function useActiveAccount(
  walletId: string | null,
  accounts: IPC.AccountInfo[]
): { activeAccountId: number | null; setActiveAccountId: (accountId: number) => void } {
  const [activeAccountId, setActiveAccountIdState] = useState<number | null>(null);

  const sortedAccountIds = useMemo(
    () => accounts.map((a) => a.id).slice().sort((a, b) => a - b),
    [accounts]
  );

  useEffect(() => {
    if (!walletId) {
      setActiveAccountIdState(null);
      return;
    }

    const persisted = window.localStorage.getItem(storageKey(walletId));
    const persistedId = persisted ? Number.parseInt(persisted, 10) : null;
    const persistedIsValid =
      persistedId !== null && Number.isFinite(persistedId) && accounts.some((a) => a.id === persistedId);

    if (persistedIsValid) {
      setActiveAccountIdState(persistedId);
      return;
    }

    if (accounts.some((a) => a.id === 0)) {
      setActiveAccountIdState(0);
      return;
    }

    setActiveAccountIdState(sortedAccountIds.length > 0 ? sortedAccountIds[0] : null);
  }, [walletId, accounts, sortedAccountIds]);

  const setActiveAccountId = (accountId: number) => {
    setActiveAccountIdState(accountId);
    if (walletId) {
      window.localStorage.setItem(storageKey(walletId), String(accountId));
    }
  };

  return { activeAccountId, setActiveAccountId };
}

