import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import * as IPC from '../types/ipc';
import { buildSigningRequest, prepareSend } from '../services/ipc';

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

  const canSubmit = useMemo(() => {
    if (activeAccount == null) return false;
    if (!recipient.trim()) return false;
    if (!amount.trim()) return false;
    if (transparentRecipient && !transparentAck) return false;
    return true;
  }, [activeAccount, recipient, amount, transparentRecipient, transparentAck]);

  const submit = async () => {
    if (activeAccount == null) return;

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
        amount,
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
      amount,
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
    <form
      style={{ display: 'grid', gap: 12, maxWidth: 560 }}
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <h1>Send</h1>
      <label style={{ display: 'grid', gap: 4 }}>
        <span>Recipient</span>
        <input
          value={recipient}
          onChange={(e) => {
            setRecipient(e.currentTarget.value);
            setTransparentRecipient(false);
            setTransparentAck(false);
            setError(null);
          }}
          placeholder="UA / Sapling / transparent address"
        />
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Amount (zatoshis)</span>
        <input value={amount} onChange={(e) => setAmount(e.currentTarget.value)} />
      </label>

      <label style={{ display: 'grid', gap: 4 }}>
        <span>Memo (optional)</span>
        <textarea
          value={memo}
          onChange={(e) => setMemo(e.currentTarget.value)}
          disabled={transparentRecipient}
          rows={3}
          placeholder={transparentRecipient ? 'Disabled for transparent recipients' : 'Memo (<=512 bytes)'}
        />
      </label>

      {transparentRecipient ? (
        <label style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <input
            type="checkbox"
            checked={transparentAck}
            onChange={(e) => setTransparentAck(e.currentTarget.checked)}
          />
          <span>
            I understand sending to a transparent address reduces privacy.
          </span>
        </label>
      ) : null}

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <button type="submit" disabled={!canSubmit || submitting}>
        {submitting ? 'Preparing…' : 'Review'}
      </button>
    </form>
  );
}
