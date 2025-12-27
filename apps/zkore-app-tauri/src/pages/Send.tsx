import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import * as IPC from '../types/ipc';
import { prepareSend } from '../services/ipc';

export function Send(props: { activeAccountId: number | null }) {
  const { activeAccountId } = props;
  const navigate = useNavigate();

  const [recipient, setRecipient] = useState('');
  const [amount, setAmount] = useState('');
  const [memo, setMemo] = useState('');
  const [transparentAck, setTransparentAck] = useState(false);
  const [transparentRecipient, setTransparentRecipient] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const canSubmit = useMemo(() => {
    if (activeAccountId == null) return false;
    if (!recipient.trim()) return false;
    if (!amount.trim()) return false;
    if (transparentRecipient && !transparentAck) return false;
    return true;
  }, [activeAccountId, recipient, amount, transparentRecipient, transparentAck]);

  const submit = async () => {
    if (activeAccountId == null) return;

    setSubmitting(true);
    setError(null);

    const allowTransparent = transparentRecipient && transparentAck;
    const memoValue = memo.trim() ? memo : null;

    const res = await prepareSend({
      account_id: activeAccountId,
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
    <div style={{ display: 'grid', gap: 12, maxWidth: 560 }}>
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

      <button type="button" onClick={submit} disabled={!canSubmit || submitting}>
        {submitting ? 'Preparing…' : 'Review'}
      </button>
    </div>
  );
}
