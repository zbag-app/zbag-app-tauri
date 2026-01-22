import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { ArrowUp, ArrowDown, Shield, ArrowLeftRight } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { ShieldPrompt } from '../components/wallet/ShieldPrompt';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Badge } from '../components/ui/badge';
import { onBalanceChanged } from '../services/events';
import { getBalance, getWalletStatus } from '../services/ipc';
import { formatZatoshisToZec, formatFiat, zatoshisToFiat } from '../utils/zec';
import { cn } from '../lib/utils';
import { useFiatDisplay } from '../hooks/useFiatDisplay';

export function Home(props: {
  wallet: IPC.WalletInfo;
  activeAccountId: number | null;
}) {
  const { wallet, activeAccountId } = props;

  const [status, setStatus] = useState<IPC.WalletStatus | null>(null);
  const [balance, setBalance] = useState<IPC.Balance | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Use centralized fiat display hook
  const {
    settings: fiatSettings,
    rate: exchangeRate,
    refreshError: fiatRefreshError,
    refreshCooldownSecs,
    refreshRate,
    loading: fiatLoading,
  } = useFiatDisplay();

  const refreshBalance = async () => {
    if (activeAccountId === null) {
      setBalance(null);
      return;
    }
    const res = await getBalance({ account_id: activeAccountId });
    if ('err' in res) {
      setError(res.err.message);
      return;
    }
    setBalance(res.ok.balance);
  };

  // Fetch wallet status
  useEffect(() => {
    let cancelled = false;

    async function run() {
      const res = await getWalletStatus({ wallet_id: wallet.id });
      if (cancelled) return;
      if ('ok' in res) {
        setStatus(res.ok.status);
      }
    }

    run();
    return () => {
      cancelled = true;
    };
  }, [wallet.id]);

  // Fetch balance
  useEffect(() => {
    let cancelled = false;

    async function run() {
      if (activeAccountId === null) {
        setBalance(null);
        return;
      }
      const res = await getBalance({ account_id: activeAccountId });
      if (cancelled) return;
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setBalance(res.ok.balance);
    }

    run();
    return () => {
      cancelled = true;
    };
  }, [activeAccountId]);

  // Balance change subscription
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    onBalanceChanged((evt) => {
      if (activeAccountId === null) return;
      if (evt.account_id !== activeAccountId) return;
      setBalance(evt.balance);
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, [activeAccountId]);

  const backupRequired = status?.backup_status === 'Required';
  const needsShielding = balance?.transparent_total !== undefined && balance.transparent_total !== '0';

  // Calculate total balance
  const totalZec = balance ? formatZatoshisToZec(balance.total) : '0';
  const [wholePart, decimalPart] = totalZec.split('.');

  // Calculate fiat value if enabled
  const showFiat = fiatSettings?.enabled && exchangeRate;
  const totalFiat = showFiat && balance ? zatoshisToFiat(balance.total, exchangeRate.price) : null;
  const shieldedFiat = showFiat && balance ? zatoshisToFiat(balance.shielded_spendable, exchangeRate.price) : null;
  const transparentFiat = showFiat && balance ? zatoshisToFiat(balance.transparent_total, exchangeRate.price) : null;

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      {error && (
        <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      {/* Balance Hero */}
      <Card className="overflow-hidden">
        <CardContent className="pt-6">
          <div className="text-center">
            <p className="text-sm text-muted-foreground mb-2">Total Balance</p>
            {balance ? (
              <>
                <div className="flex items-baseline justify-center gap-1">
                  <span className="text-5xl font-bold balance-number">{wholePart}</span>
                  <span className="text-2xl text-muted-foreground">.{decimalPart || '00'}</span>
                  <span className="text-xl text-muted-foreground ml-2">ZEC</span>
                </div>
                {showFiat && totalFiat !== null && (
                  <p className="text-lg text-muted-foreground mt-1">
                    {formatFiat(totalFiat, exchangeRate.currency)}
                  </p>
                )}
                {fiatSettings?.enabled && !exchangeRate && (
                  <div className="text-sm text-muted-foreground mt-2">
                    {fiatLoading ? (
                      <span>Fetching exchange rate...</span>
                    ) : (
                      <span>
                        Fiat rate unavailable
                        {refreshCooldownSecs > 0 ? ` (retry in ${refreshCooldownSecs}s)` : ''}.
                      </span>
                    )}
                    {fiatRefreshError ? (
                      <span className="block text-xs text-muted-foreground mt-1">{fiatRefreshError}</span>
                    ) : null}
                    <Button
                      variant="outline"
                      size="sm"
                      className="mt-2"
                      onClick={() => refreshRate(true)}
                      disabled={fiatLoading}
                    >
                      Retry
                    </Button>
                  </div>
                )}
              </>
            ) : (
              <div className="text-2xl text-muted-foreground">
                {activeAccountId === null ? 'No active account' : 'Loading...'}
              </div>
            )}
          </div>

          {/* Quick Actions */}
          <div className="grid grid-cols-4 gap-3 mt-6">
            <Link to="/send">
              <Button variant="secondary" className="w-full flex-col h-auto py-3 gap-1">
                <ArrowUp className="h-5 w-5" />
                <span className="text-xs">Send</span>
              </Button>
            </Link>
            <Link to="/receive">
              <Button variant="secondary" className="w-full flex-col h-auto py-3 gap-1">
                <ArrowDown className="h-5 w-5" />
                <span className="text-xs">Receive</span>
              </Button>
            </Link>
            <Link to="/swap">
              <Button variant="secondary" className="w-full flex-col h-auto py-3 gap-1">
                <ArrowLeftRight className="h-5 w-5" />
                <span className="text-xs">Swap</span>
              </Button>
            </Link>
            {needsShielding && activeAccountId !== null && wallet.wallet_type !== 'WatchOnly' ? (
              <ShieldPrompt
                walletId={wallet.id}
                accountId={activeAccountId}
                transparentTotal={balance?.transparent_total ?? '0'}
                disabled={backupRequired}
                onShielded={refreshBalance}
              />
            ) : (
              <Button
                variant="secondary"
                className="w-full flex-col h-auto py-3 gap-1"
                disabled
                title={wallet.wallet_type === 'WatchOnly' ? 'Shielding not available for hardware wallets' : undefined}
              >
                <Shield className="h-5 w-5" />
                <span className="text-xs">Shield</span>
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Balance Breakdown */}
      <div className="grid grid-cols-2 gap-4">
        <Card className={cn("glow-shielded", needsShielding ? "" : "")}>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <div className="h-2 w-2 rounded-full bg-shielded" />
              Shielded
            </CardTitle>
          </CardHeader>
          <CardContent>
            {balance ? (
              <div className="space-y-1">
                <div className="text-xl font-semibold balance-number">
                  {formatZatoshisToZec(balance.shielded_spendable)} <span className="text-sm text-muted-foreground">ZEC</span>
                </div>
                {showFiat && shieldedFiat !== null && (
                  <div className="text-xs text-muted-foreground">
                    {formatFiat(shieldedFiat, exchangeRate.currency)}
                  </div>
                )}
                {balance.shielded_pending !== '0' && (
                  <div className="text-xs text-muted-foreground">
                    +{formatZatoshisToZec(balance.shielded_pending)} pending
                  </div>
                )}
              </div>
            ) : (
              <div className="text-muted-foreground">-</div>
            )}
          </CardContent>
        </Card>

        <Card className={cn(needsShielding ? "border-warning/50" : "")}>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <div className="h-2 w-2 rounded-full bg-transparent-pool" />
              Transparent
              {needsShielding && (
                <Badge variant="warning" className="ml-auto text-[10px]">
                  Needs Shielding
                </Badge>
              )}
            </CardTitle>
          </CardHeader>
          <CardContent>
            {balance ? (
              <div className="space-y-1">
                <div className="text-xl font-semibold balance-number">
                  {formatZatoshisToZec(balance.transparent_total)} <span className="text-sm text-muted-foreground">ZEC</span>
                </div>
                {showFiat && transparentFiat !== null && (
                  <div className="text-xs text-muted-foreground">
                    {formatFiat(transparentFiat, exchangeRate.currency)}
                  </div>
                )}
              </div>
            ) : (
              <div className="text-muted-foreground">-</div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Backup Warning */}
      {backupRequired && (
        <Card className="border-warning/50 bg-warning/5">
          <CardContent className="pt-6">
            <div className="flex items-start gap-4">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-none bg-warning/20">
                <Shield className="h-5 w-5 text-warning" />
              </div>
              <div className="flex-1">
                <h3 className="font-semibold text-warning">Backup Required</h3>
                <p className="text-sm text-muted-foreground mt-1">
                  Your wallet is not backed up. Please write down your seed phrase to protect your funds.
                </p>
                <Link to="/backup/flow">
                  <Button variant="outline" size="sm" className="mt-3">
                    Backup Now
                  </Button>
                </Link>
              </div>
            </div>
          </CardContent>
        </Card>
      )}
    </div>
  );
}
