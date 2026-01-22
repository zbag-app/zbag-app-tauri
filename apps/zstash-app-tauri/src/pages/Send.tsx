import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowUp, AlertTriangle } from 'lucide-react';
import * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { buildSigningRequest, prepareSend } from '../services/ipc';
import { parseZecToZatoshis, formatFiat, zatoshisToFiat } from '../utils/zec';
import { useFiatDisplayContext } from '../context/FiatDisplayContext';

export function Send(props: { activeAccount: IPC.AccountInfo | null }) {
  const { activeAccount } = props;
  const navigate = useNavigate();

  const [recipient, setRecipient] = useState('');
  const [amount, setAmount] = useState('');
  const [memo, setMemo] = useState('');
  const [transparentAck, setTransparentAck] = useState(false);
  const [transparentRecipient, setTransparentRecipient] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // Use centralized fiat display context
  const { settings: fiatSettings, rate: exchangeRate } = useFiatDisplayContext();

  const parsedAmount = useMemo(() => parseZecToZatoshis(amount), [amount]);
  const amountZatoshis = 'ok' in parsedAmount ? parsedAmount.ok : null;
  const amountError = useMemo(() => {
    if (!amount.trim()) return null;
    if ('err' in parsedAmount) return parsedAmount.err;
    return null;
  }, [amount, parsedAmount]);

  const canSubmit = useMemo(() => {
    if (activeAccount == null) return false;
    if (!recipient.trim()) return false;
    if (!amountZatoshis) return false;
    if (transparentRecipient && !transparentAck) return false;
    return true;
  }, [activeAccount, recipient, amountZatoshis, transparentRecipient, transparentAck]);

  const submit = async () => {
    if (activeAccount == null) return;
    if (!amountZatoshis) {
      setError('Enter a valid amount.');
      return;
    }

    setSubmitting(true);
    setError(null);

    const allowTransparent = transparentRecipient && transparentAck;
    const memoValue = memo.trim() ? memo : null;

    const accountId = activeAccount.id;
    const isHardwareSigner = activeAccount.account_type === 'HardwareSigner';

    if (isHardwareSigner) {
      const res = await buildSigningRequest({
        account_id: accountId,
        recipient,
        amount: amountZatoshis,
        memo: memoValue,
        allow_transparent_recipient: allowTransparent,
      });

      setSubmitting(false);

      if ('err' in res) {
        if (res.err.code === IPC.ErrorCodes.PRIVACY_ACK_REQUIRED) {
          setTransparentRecipient(true);
          setMemo('');
          setError(
            'This recipient is transparent. Confirm the privacy acknowledgement to continue.'
          );
          return;
        }

        if (res.err.code === IPC.ErrorCodes.MEMO_NOT_ALLOWED) {
          setTransparentRecipient(true);
          setMemo('');
          setError('Memos are not allowed for transparent recipients.');
          return;
        }

        setError(res.err.message);
        return;
      }

      navigate('/signing', {
        state: {
          signingRequest: res.ok.signing_request,
        },
      });
      return;
    }

    const res = await prepareSend({
      account_id: accountId,
      recipient,
      amount: amountZatoshis,
      memo: memoValue,
      allow_transparent_recipient: allowTransparent,
    });

    setSubmitting(false);

    if ('err' in res) {
      if (res.err.code === IPC.ErrorCodes.PRIVACY_ACK_REQUIRED) {
        setTransparentRecipient(true);
        setMemo('');
        setError('This recipient is transparent. Confirm the privacy acknowledgement to continue.');
        return;
      }

      if (res.err.code === IPC.ErrorCodes.MEMO_NOT_ALLOWED) {
        setTransparentRecipient(true);
        setMemo('');
        setError('Memos are not allowed for transparent recipients.');
        return;
      }

      setError(res.err.message);
      return;
    }

    navigate('/send/confirm', {
      state: {
        proposal: res.ok,
      },
    });
  };

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <ArrowUp className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Send</h1>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Transaction Details</CardTitle>
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
              <Label htmlFor="recipient">Recipient</Label>
              <Input
                id="recipient"
                value={recipient}
                onChange={(e) => {
                  setRecipient(e.currentTarget.value);
                  setTransparentRecipient(false);
                  setTransparentAck(false);
                  setError(null);
                }}
                placeholder="UA / Sapling / transparent address"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="amount">Amount (ZEC)</Label>
              <Input
                id="amount"
                value={amount}
                onChange={(e) => {
                  setAmount(e.currentTarget.value);
                  setError(null);
                }}
                inputMode="decimal"
                placeholder="0.12345678"
              />
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">Up to 8 decimal places</span>
                {fiatSettings?.enabled && exchangeRate && amountZatoshis && (
                  <span className="text-muted-foreground">
                    {formatFiat(zatoshisToFiat(amountZatoshis, exchangeRate.price), exchangeRate.currency)}
                  </span>
                )}
              </div>
              {amountError && (
                <p className="text-xs text-destructive">{amountError}</p>
              )}
            </div>

            <div className="space-y-2">
              <Label htmlFor="memo">Memo (optional)</Label>
              <textarea
                id="memo"
                value={memo}
                onChange={(e) => setMemo(e.currentTarget.value)}
                disabled={transparentRecipient}
                rows={3}
                placeholder={transparentRecipient ? 'Disabled for transparent recipients' : 'Memo (<=512 bytes)'}
                className="flex w-full rounded-none border border-border bg-input px-3 py-2 text-sm text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
              />
            </div>

            {transparentRecipient && (
              <div className="flex items-start gap-3 rounded-none border border-warning/50 bg-warning/5 p-3">
                <AlertTriangle className="h-4 w-4 text-warning shrink-0 mt-0.5" />
                <label className="flex items-center gap-3 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={transparentAck}
                    onChange={(e) => setTransparentAck(e.currentTarget.checked)}
                    className="rounded border-border h-4 w-4 accent-primary"
                  />
                  <span className="text-sm">
                    I understand sending to a transparent address reduces privacy
                  </span>
                </label>
              </div>
            )}

            {error && (
              <div className="rounded-none border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
                {error}
              </div>
            )}

            <Button type="submit" disabled={!canSubmit || submitting} className="w-full">
              {submitting ? 'Preparing...' : 'Review'}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
