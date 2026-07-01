import * as React from "react";
import { Sidebar } from "./Sidebar";
import type { WalletInfo, TorState, SyncProgress } from "../../types/ipc";

interface AppShellProps {
  wallet: WalletInfo;
  torState: TorState | null;
  syncProgress: SyncProgress | null;
  onLock: () => void;
  children: React.ReactNode;
}

export function AppShell({
  wallet,
  torState,
  syncProgress,
  onLock,
  children,
}: AppShellProps) {
  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar
        walletName={wallet.name}
        network={wallet.network}
        torState={torState}
        syncProgress={syncProgress}
        onLock={onLock}
      />
      <main className="flex-1 overflow-auto">
        <div className="mx-auto max-w-4xl p-8">{children}</div>
      </main>
    </div>
  );
}
