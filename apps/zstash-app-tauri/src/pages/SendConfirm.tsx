import { useEffect, useMemo, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { AlertTriangle, Send, CheckCircle } from 'lucide-react';
import * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { Badge } from '../components/ui/badge';
import { cancelSend, confirmSend, reauthWallet } from '../services/ipc';
import { formatZatoshisToZec } from '../utils/zec';

type LocationState = {
  proposal: IPC.PrepareSendResponse;
};

export function SendConfirm(props: { walletId: string }) {
  const { walletId } = props;
  const navigate = useNavigate();
  const location = useLocation();

  const [proposal] = useState<IPC.PrepareSendResponse | null>(() => {
    const state = location.state as LocationState | null;
    return state?.proposal ?? null;
  });
  const summary = proposal?.summary ?? null;

  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (location.state != null) {
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.pathname, location.state, navigate]);

  const warning = useMemo(() => {
    if (!summary) return null;
    if (summary.recipient_kind !== 'Transparent') return null;
    return 'Transparent recipients reduce privacy. Memos are not supported.';
  }, [summary]);

  const submit = async () => {
    if (!proposal) return;

    setSubmitting(true);
    setError(null);

    const reauth = await reauthWallet({ wallet_id: walletId, password, purpose: 'Spend' });
    if ('err' in reauth) {
      setSubmitting(false);
      setError(reauth.err.message);
      return;
    }

    const res = await confirmSend({
      proposal_id: proposal.proposal_id,
      reauth_token: reauth.ok.reauth_token,
    });
    setSubmitting(false);

    if ('err' in res) {
      setError(res.err.message);
      return;
    }

    setPassword('');
    navigate('/activity');
  };

  const cancel = async () => {
    if (proposal) {
      await cancelSend({ proposal_id: proposal.proposal_id });
    }
    setPassword('');
    navigate('/send');
  };

  if (!proposal || !summary) {
    return (
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <Send className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Review Send</h1>
        </div>
        <Card>
          <CardContent className="pt-6">
            <p className="text-muted-foreground">
              Missing proposal. Return to <Link to="/send" className="text-primary hover:underline">Send</Link>.
            </p>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <Send className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Review Send</h1>
      </div>

      {warning && (
        <div className="flex items-start gap-3 rounded-none border border-warning/50 bg-warning/5 p-4">
          <AlertTriangle className="h-5 w-5 text-warning shrink-0 mt-0.5" />
          <div>
            <h3 className="font-semibold text-warning">Privacy Warning</h3>
            <p className="text-sm text-muted-foreground mt-1">{warning}</p>
          </div>
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Transaction Summary</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-1">
            <span className="text-sm text-muted-foreground">Recipient</span>
            <code className="block text-sm break-all bg-muted px-3 py-2 rounded-none font-mono">
              {summary.recipient}
            </code>
          </div>

          <div className="grid grid-cols-2 gap-4 text-sm">
            <div className="space-y-1">
              <span className="text-muted-foreground">Recipient Kind</span>
              <div className="flex">
                <Badge variant={summary.recipient_kind === 'Transparent' ? 'warning' : 'shielded'}>
                  {summary.recipient_kind}
                </Badge>
              </div>
            </div>
            <div className="space-y-1">
              <span className="text-muted-foreground">Amount</span>
              <div className="font-semibold balance-number">{formatZatoshisToZec(summary.amount)} ZEC</div>
            </div>
            <div className="space-y-1">
              <span className="text-muted-foreground">Fee</span>
              <div className="font-semibold balance-number">{formatZatoshisToZec(summary.fee)} ZEC</div>
            </div>
            <div className="space-y-1">
              <span className="text-muted-foreground">Total Spend</span>
              <div className="font-semibold balance-number">{formatZatoshisToZec(summary.total_spend)} ZEC</div>
            </div>
            <div className="space-y-1">
              <span className="text-muted-foreground">Memo</span>
              <div className="flex">
                {summary.memo_present ? (
                  <Badge variant="success"><CheckCircle className="h-3 w-3 mr-1" />Yes</Badge>
                ) : (
                  <Badge variant="secondary">No</Badge>
                )}
              </div>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Confirm Transaction</CardTitle>
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
              <Label htmlFor="password">Password</Label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.currentTarget.value)}
                disabled={submitting}
                placeholder="Enter your password to confirm"
              />
            </div>

            {error && (
              <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                {error}
              </div>
            )}

            <div className="flex gap-3">
              <Button type="submit" disabled={!password || submitting} className="flex-1">
                {submitting ? 'Sending...' : 'Confirm & Send'}
              </Button>
              <Button type="button" variant="outline" onClick={cancel} disabled={submitting}>
                Cancel
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
