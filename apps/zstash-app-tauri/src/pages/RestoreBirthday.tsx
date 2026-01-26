import { useEffect, useRef, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { Calendar, RotateCcw, ArrowLeft } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { loadWallet, restoreWallet, startSync } from '../services/ipc';
import type { RestoreFlowData } from './RestoreWallet';

type LocationState = RestoreFlowData;

export function RestoreBirthday(props: {
  onRestored: (args: { wallet: IPC.WalletInfo; accounts: IPC.AccountInfo[] }) => void;
}) {
  const { onRestored } = props;
  const navigate = useNavigate();
  const location = useLocation();

  // Read flow data from navigation state.
  const [flow, setFlow] = useState<RestoreFlowData | null>(() => {
    const state = location.state as LocationState | null;
    return state ?? null;
  });

  // Clear navigation state from the history entry after reading it.
  const clearedRef = useRef(false);

  useEffect(() => {
    if (location.state != null && !clearedRef.current) {
      clearedRef.current = true;
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.pathname, location.state, navigate]);

  // Defense-in-depth: explicitly clear sensitive flow data (contains mnemonic)
  // on unmount to minimize memory retention window. While React will GC the
  // component state, this ensures no closures retain references to the mnemonic.
  useEffect(() => {
    return () => {
      setFlow(null);
    };
  }, []);

  const [birthdayDate, setBirthdayDate] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const goBack = () => {
    setFlow(null);
    setBirthdayDate('');
    navigate('/restore');
  };

  const submit = async () => {
    if (!flow) return;
    setError(null);

    let birthdayMs: number | null = null;
    if (birthdayDate) {
      birthdayMs = Date.parse(birthdayDate);
      if (Number.isNaN(birthdayMs)) {
        setError('Invalid date.');
        return;
      }
    }

    setSubmitting(true);
    try {
      const restored = await restoreWallet({ ...flow, birthday_date: birthdayMs });
      if ('err' in restored) {
        setError(restored.err.message);
        return;
      }

      const walletId = restored.ok.wallet.id;

      const load = await loadWallet({ wallet_id: walletId });
      if ('err' in load) {
        setError(load.err.message);
        return;
      }

      setFlow(null);
      onRestored({ wallet: load.ok.wallet, accounts: load.ok.accounts });

      await startSync({ wallet_id: walletId });
      navigate('/');
    } finally {
      setSubmitting(false);
    }
  };

  if (!flow) {
    return (
      <div className="flex min-h-screen items-center justify-center p-4">
        <Card className="w-full max-w-lg animate-[scale-in_0.3s_ease-out]">
          <CardContent className="pt-6">
            <p className="text-muted-foreground">Restore details are missing.</p>
            <Link to="/restore" className="text-sm text-primary hover:underline mt-4 inline-block">
              Start restore
            </Link>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-4">
      <Card className="w-full max-w-lg animate-[scale-in_0.3s_ease-out]">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-2xl bg-primary/10">
            <Calendar className="h-8 w-8 text-primary" />
          </div>
          <CardTitle className="font-display text-2xl">First Transaction Date</CardTitle>
          <p className="text-sm text-muted-foreground">
            If you remember roughly when this wallet first received funds, adding a date can reduce
            restore scan time.
          </p>
        </CardHeader>
        <CardContent>
          <form
            className="space-y-4"
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
          >
            <div className="space-y-2">
              <Label htmlFor="birthdayDate">Date (optional)</Label>
              <Input
                id="birthdayDate"
                type="date"
                value={birthdayDate}
                onChange={(e) => setBirthdayDate(e.currentTarget.value)}
              />
            </div>

            {error && (
              <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                {error}
              </div>
            )}

            <div className="flex gap-3">
              <Button type="button" variant="outline" onClick={goBack} disabled={submitting}>
                <ArrowLeft className="h-4 w-4" />
                Back
              </Button>
              <Button type="submit" disabled={submitting} className="flex-1">
                <RotateCcw className="h-4 w-4" />
                {submitting ? 'Restoring...' : 'Restore'}
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
