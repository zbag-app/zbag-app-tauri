import { useEffect, useState } from 'react';
import type * as IPC from '../types/ipc';
import type { FiatCurrency } from '../types/ipc';
import { Link } from 'react-router-dom';
import { Settings as SettingsIcon, Server, Shield, Key, FileText, ChevronRight, Info, DollarSign, AlertTriangle } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Badge } from '../components/ui/badge';
import { Button } from '../components/ui/button';
import { ViewSeedPhraseDialog } from '../components/common/ViewSeedPhraseDialog';
import { FiatCurrencySelect } from '../components/ui/FiatCurrencySelect';
import { LogoutDialog } from '../components/common/LogoutDialog';
import { getLogLocation, getVersion } from '../services/ipc';
import { useFiatDisplayContext } from '../context/FiatDisplayContext';

export function Settings(props: {
  wallet: IPC.WalletInfo;
  torState: IPC.TorState | null;
  onSetTorEnabled: (enabled: boolean) => void;
  onLogout: () => void;
}) {
  const { wallet, torState, onSetTorEnabled, onLogout } = props;
  const [logLocation, setLogLocation] = useState<IPC.GetLogLocationResponse | null>(null);
  const [logError, setLogError] = useState<string | null>(null);
  const [versionInfo, setVersionInfo] = useState<IPC.VersionInfo | null>(null);

  // Fiat display context
  const {
    settings: fiatSettings,
    error: fiatError,
    loading: fiatSaving,
    updateSettings: updateFiatSettings,
  } = useFiatDisplayContext();

  // Local UI state for privacy warning dialog
  const [showPrivacyWarning, setShowPrivacyWarning] = useState(false);
  const [pendingFiatEnabled, setPendingFiatEnabled] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function run() {
      const [logRes, versionRes] = await Promise.all([getLogLocation(), getVersion()]);
      if (cancelled) return;

      if ('err' in logRes) {
        setLogError(logRes.err.message);
      } else {
        setLogLocation(logRes.ok);
      }

      if ('ok' in versionRes) {
        setVersionInfo(versionRes.ok.version_info);
      } else {
        console.warn('Failed to fetch version info:', versionRes.err.message);
      }
    }

    run();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleFiatToggle = async (enabled: boolean) => {
    if (enabled && !fiatSettings?.privacy_acknowledged) {
      // Show privacy warning before enabling
      setPendingFiatEnabled(true);
      setShowPrivacyWarning(true);
      return;
    }

    await updateFiatSettings(
      enabled,
      fiatSettings?.currency ?? 'USD',
      fiatSettings?.privacy_acknowledged ?? false
    );
  };

  const handlePrivacyAcknowledge = async () => {
    const success = await updateFiatSettings(
      pendingFiatEnabled,
      fiatSettings?.currency ?? 'USD',
      true
    );
    if (success) {
      setShowPrivacyWarning(false);
    }
  };

  const handleCurrencyChange = async (currency: IPC.FiatCurrency) => {
    if (!fiatSettings) return;

    await updateFiatSettings(
      fiatSettings.enabled,
      currency,
      fiatSettings.privacy_acknowledged
    );
  };

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
          <Link to="/settings/servers" className="flex items-center justify-between p-3 -mx-3 rounded-none hover:bg-accent transition-colors">
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
            Opt-in Tor anonymization for all network traffic. When enabled, bagZ fails closed if Tor is not healthy.
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
            <div className="rounded-none border border-warning/50 bg-warning/5 p-3 text-sm text-warning">
              Status: {torState.status}. Some operations may be blocked until Tor is connected.
            </div>
          )}
        </CardContent>
      </Card>

      {/* Fiat Display Section */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="text-lg flex items-center gap-2">
              <DollarSign className="h-4 w-4" />
              Fiat Display
            </CardTitle>
            {fiatSettings?.enabled && <Badge variant="success">On</Badge>}
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Show approximate fiat equivalents for balances and transaction amounts.
          </p>

          {/* Privacy Warning Dialog */}
          {showPrivacyWarning && (
            <div className="rounded-lg border border-warning/50 bg-warning/5 p-4 space-y-4">
              <div className="flex items-start gap-3">
                <AlertTriangle className="h-5 w-5 text-warning shrink-0 mt-0.5" />
                <div className="space-y-3">
                  <h4 className="font-semibold text-warning">Privacy Notice</h4>
                  <div className="text-sm space-y-2">
                    <p>
                      Enabling fiat display requires fetching exchange rates from third-party services.
                    </p>
                    {torState?.enabled ? (
                      <p>
                        Exchange rates are fetched over Tor to protect your IP address. Ensure Tor use is allowed in your region.
                      </p>
                    ) : (
                      <p className="text-destructive">
                        <strong>Warning:</strong> Tor is currently disabled. Exchange rate requests will expose your IP address to the rate provider.
                      </p>
                    )}
                    <p className="text-muted-foreground">
                      Because we pull the conversion rate from exchanges, an exchange might be able to see that the exchange rate was queried before a transaction occurred.
                    </p>
                  </div>
                  <div className="flex gap-2 pt-2">
                    <Button
                      size="sm"
                      onClick={handlePrivacyAcknowledge}
                      disabled={fiatSaving}
                    >
                      {fiatSaving ? 'Enabling...' : 'I Understand, Enable'}
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        setShowPrivacyWarning(false);
                        setPendingFiatEnabled(false);
                      }}
                      disabled={fiatSaving}
                    >
                      Cancel
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {!showPrivacyWarning && (
            <>
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={fiatSettings?.enabled ?? false}
                  onChange={(e) => handleFiatToggle(e.currentTarget.checked)}
                  disabled={fiatSaving}
                  className="rounded border-border h-4 w-4 accent-primary"
                  aria-label="Show fiat values"
                />
                <span className="text-sm">Show fiat values</span>
              </label>

              {fiatSettings?.enabled && (
                <div className="space-y-2">
                  <label className="text-sm text-muted-foreground">Currency</label>
                  <FiatCurrencySelect
                    value={fiatSettings.currency}
                    onChange={(currency: FiatCurrency) => handleCurrencyChange(currency)}
                    disabled={fiatSaving}
                  />
                </div>
              )}

              {fiatSettings?.enabled && !torState?.enabled && (
                <div className="rounded-lg border border-warning/50 bg-warning/5 p-3 text-sm text-warning">
                  <div className="flex items-start gap-2">
                    <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
                    <span>
                      Tor is disabled. Exchange rate requests will expose your IP address to the rate provider.
                    </span>
                  </div>
                </div>
              )}
            </>
          )}

          {fiatError && (
            <div className="text-sm text-destructive">{fiatError}</div>
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

      {/* About Section */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            <Info className="h-4 w-4" />
            About
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2 text-sm">
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">Version</span>
              <span className="font-mono">{versionInfo?.full_version ?? '-'}</span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-muted-foreground">Built</span>
              <span className="font-mono text-xs">{versionInfo?.build_timestamp ?? '-'}</span>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
