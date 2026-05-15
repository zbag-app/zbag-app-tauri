import { Link, useLocation } from "react-router-dom";
import {
  Home,
  Send,
  Download,
  ArrowLeftRight,
  History,
  Settings,
  Lock,
} from "lucide-react";
import { Logo } from "../brand/Logo";
import { cn } from "../../lib/utils";
import { Badge } from "../ui/badge";
import { Progress } from "../ui/progress";
import { Separator } from "../ui/separator";
import type { TorState, SyncProgress } from "../../types/ipc";
import { formatEta } from "../../utils/time";
import { getDisplaySyncPercent, isEffectivelyAtTip } from "../../utils/sync";

interface SidebarProps {
  walletName: string;
  network: "Mainnet" | "Testnet";
  torState: TorState | null;
  syncProgress: SyncProgress | null;
  onLock: () => void;
}

const navItems = [
  { to: "/", icon: Home, label: "Home" },
  { to: "/send", icon: Send, label: "Send" },
  { to: "/receive", icon: Download, label: "Receive" },
  { to: "/swap", icon: ArrowLeftRight, label: "Swap" },
  { to: "/activity", icon: History, label: "Activity" },
  { to: "/settings", icon: Settings, label: "Settings" },
];

function getSyncLabel(progress: SyncProgress): string {
  switch (progress.phase) {
    case "Downloading":
    case "Scanning":
      return "Syncing";
    case "CatchingUp":
      return isEffectivelyAtTip(progress) ? "Synced" : "Catching up";
    case "Idle":
      return "Synced";
    case "Offline": {
      const retryInSeconds = progress.retry_in_seconds;
      if (retryInSeconds == null) return "Offline";
      if (retryInSeconds <= 0) return "Offline (retrying...)";
      return `Offline (retry in ${formatEta(retryInSeconds)})`;
    }
    case "Error": {
      const retryInSeconds = progress.retry_in_seconds;
      const details = progress.error_message ? `: ${progress.error_message}` : "";
      if (retryInSeconds == null) return `Error${details}`;
      if (retryInSeconds <= 0) return `Error${details} (retrying...)`;
      return `Error${details} (retry in ${formatEta(retryInSeconds)})`;
    }
    default:
      return progress.phase;
  }
}

export function Sidebar({
  walletName,
  network,
  torState,
  syncProgress,
  onLock,
}: SidebarProps) {
  const location = useLocation();

  const getTorStatusColor = () => {
    if (!torState?.enabled) return "bg-muted-foreground";
    switch (torState.status) {
      case "On":
        return "bg-success";
      case "Connecting":
        return "bg-warning animate-pulse";
      case "Error":
        return "bg-destructive";
      default:
        return "bg-muted-foreground";
    }
  };

  const getTorStatusText = () => {
    if (!torState?.enabled) return "Off";
    switch (torState.status) {
      case "On":
        return "Connected";
      case "Connecting":
        return "Connecting";
      case "Error":
        return "Error";
      default:
        return "Off";
    }
  };

  const displayPercent = syncProgress ? getDisplaySyncPercent(syncProgress) : 0;

  return (
    <aside className="flex h-screen w-64 flex-col border-r border-border bg-card">
      {/* Logo Section */}
      <div className="flex items-center gap-3 p-6">
        <Logo size={40} />
        <div className="flex flex-col">
          <span className="font-display text-lg font-bold tracking-tight">bagZ</span>
          <div className="flex items-center gap-2">
            <span className="text-xs text-muted-foreground truncate max-w-[100px]">{walletName}</span>
            <Badge variant={network === "Mainnet" ? "success" : "warning"} className="text-[10px] px-1.5 py-0">
              {network}
            </Badge>
          </div>
        </div>
      </div>

      <Separator />

      {/* Navigation */}
      <nav className="flex-1 space-y-1 p-3">
        {navItems.map((item) => {
          const isActive = location.pathname === item.to;
          return (
            <Link
              key={item.to}
              to={item.to}
              className={cn(
                "flex items-center gap-3 rounded-none px-3 py-2.5 text-sm font-medium transition-all",
                isActive
                  ? "bg-primary/10 text-primary"
                  : "text-muted-foreground hover:bg-accent hover:text-foreground"
              )}
            >
              <item.icon className="h-4 w-4" />
              {item.label}
              {isActive && (
                <div className="ml-auto h-1.5 w-1.5 rounded-full bg-primary animate-[pulse-gold_2s_ease-in-out_infinite]" />
              )}
            </Link>
          );
        })}
      </nav>

      <Separator />

      {/* Status Footer */}
      <div className="space-y-3 p-4">
        {/* Sync Progress */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between text-xs">
            <span className="text-muted-foreground">
              {syncProgress ? getSyncLabel(syncProgress) : "Sync"}
            </span>
            <span className="font-mono text-foreground">{displayPercent.toFixed(1)}%</span>
          </div>
          <Progress value={displayPercent} />
        </div>

        {/* Tor Status */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className={cn("h-2 w-2 rounded-full", getTorStatusColor())} />
            <span className="text-xs text-muted-foreground">
              Tor {getTorStatusText()}
            </span>
          </div>
        </div>

        {/* Lock Button */}
        <button
          onClick={onLock}
          className="flex w-full items-center justify-center gap-2 rounded-none border border-border bg-transparent px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <Lock className="h-4 w-4" />
          Lock Wallet
        </button>
      </div>
    </aside>
  );
}
