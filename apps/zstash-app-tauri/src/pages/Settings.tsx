import { useEffect, useState } from 'react';
import type * as IPC from '../types/ipc';
import { Link } from 'react-router-dom';
import { Settings as SettingsIcon, Server, Shield, Key, FileText, ChevronRight } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Badge } from '../components/ui/badge';
import { Button } from '../components/ui/button';
import { ViewSeedPhraseDialog } from '../components/common/ViewSeedPhraseDialog';
import { LogoutDialog } from '../components/common/LogoutDialog';
import { getLogLocation } from '../services/ipc';

export function Settings(props: {
  wallet: IPC.WalletInfo;
  torState: IPC.TorState | null;
  onSetTorEnabled: (enabled: boolean) => void;
  onLogout: () => void;
}) {
  const { wallet, torState, onSetTorEnabled, onLogout } = props;
  const [logLocation, setLogLocation] = useState<IPC.GetLogLocationResponse | null>(null);
  const [logError, setLogError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      const res = await getLogLocation();
      if (cancelled) return;
      if ('err' in res) {
        setLogError(res.err.message);
        return;
      }
      setLogLocation(res.ok);
    }

    run();
    return () => {
      cancelled = true;
    };
  }, []);

  const getTorStatusBadge = () => {
    if (!torState?.enabled) return <Badge variant="secondary">Off</Badge>;
    switch (torState.status) {
      case 'On':
        return <Badge variant="success">Connected</Badge>;
      case 'Connecting':
        return <Badge variant="warning">Connecting</Badge>;
      case 'Error':
        return <Badge variant="destructive">Error</Badge>;
      default:
        return <Badge variant="secondary">Off</Badge>;
    }
  };

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <SettingsIcon className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Settings</h1>
      </div>

      {/* Wallet Section */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            <Server className="h-4 w-4" />
            Wallet
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">Network</span>
            <Badge variant={wallet.network === 'Mainnet' ? 'success' : 'warning'}>
              {wallet.network}
            </Badge>
          </div>
          <Link to="/settings/servers" className="flex items-center justify-between p-3 -mx-3 rounded-lg hover:bg-accent transition-colors">
            <span className="text-sm">Server settings</span>
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
          </Link>
        </CardContent>
      </Card>

      {/* Tor Section */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="text-lg flex items-center gap-2">
              <Shield className="h-4 w-4" />
              Tor
            </CardTitle>
            {getTorStatusBadge()}
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Opt-in Tor anonymization for all network traffic. When enabled, zSTASH fails closed if Tor is not healthy.
          </p>
          <label className="flex items-center gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={torState?.enabled ?? false}
              onChange={(e) => onSetTorEnabled(e.currentTarget.checked)}
              className="rounded border-border h-4 w-4 accent-primary"
              aria-label="Enable Tor"
            />
            <span className="text-sm">Enable Tor (beta)</span>
          </label>
          {torState?.enabled && torState.status !== 'On' && (
            <div className="rounded-lg border border-warning/50 bg-warning/5 p-3 text-sm text-warning">
              Status: {torState.status}. Some operations may be blocked until Tor is connected.
            </div>
          )}
        </CardContent>
      </Card>

      {/* Security Section */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            <Key className="h-4 w-4" />
            Security
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Manage your wallet security settings and credentials.
          </p>
          <div className="flex flex-wrap gap-3">
            {wallet.wallet_type !== 'WatchOnly' && (
              <ViewSeedPhraseDialog walletId={wallet.id} triggerLabel="View seed phrase" />
            )}
            <LogoutDialog
              walletId={wallet.id}
              triggerLabel="Logout"
              onLogout={onLogout}
            />
          </div>
        </CardContent>
      </Card>

      {/* Hardware Wallet Section */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            <Key className="h-4 w-4" />
            Hardware Wallet
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Import a Keystone watch-only account using a UFVK.
          </p>
          <Link to="/keystone/import">
            <Button variant="outline" size="sm">
              Import Keystone UFVK
            </Button>
          </Link>
        </CardContent>
      </Card>

      {/* Logs Section */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            <FileText className="h-4 w-4" />
            Logs
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Logs are stored locally. Include the log file path when requesting support.
          </p>
          {logLocation ? (
            <div className="space-y-2 text-sm">
              <div className="flex flex-col gap-1">
                <span className="text-muted-foreground">Directory</span>
                <code className="text-xs break-all bg-muted px-2 py-1 rounded">{logLocation.log_directory}</code>
              </div>
              <div className="flex flex-col gap-1">
                <span className="text-muted-foreground">Current log</span>
                <code className="text-xs break-all bg-muted px-2 py-1 rounded">{logLocation.current_log_file}</code>
              </div>
            </div>
          ) : logError ? (
            <div className="text-sm text-destructive">{logError}</div>
          ) : (
            <div className="text-sm text-muted-foreground">Loading log location...</div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
