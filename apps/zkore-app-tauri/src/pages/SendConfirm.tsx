import { useMemo, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import * as IPC from '../types/ipc';
import { cancelSend, confirmSend, reauthWallet } from '../services/ipc';

type LocationState = {
  proposal: IPC.PrepareSendResponse;
};

export function SendConfirm(props: { walletId: string }) {
  const { walletId } = props;
  const navigate = useNavigate();
  const location = useLocation();
  const state = location.state as LocationState | null;

  const proposal = state?.proposal ?? null;
  const summary = proposal?.summary ?? null;

  const [password, setPassword] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 760 }}>
        <h1>Review send</h1>
        <div>Missing proposal. Return to <Link to="/send">Send</Link>.</div>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 760 }}>
      <h1>Review send</h1>

      {warning ? (
        <div style={{ padding: 12, border: '1px solid #c0841a', borderRadius: 8 }}>
          <strong>Privacy warning</strong>
          <div style={{ marginTop: 4 }}>{warning}</div>
        </div>
      ) : null}

      <div style={{ display: 'grid', gap: 8 }}>
        <div style={{ display: 'grid', gap: 4 }}>
          <div style={{ fontSize: 14, opacity: 0.8 }}>Recipient</div>
          <code style={{ wordBreak: 'break-all' }}>{summary.recipient}</code>
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '140px 1fr', gap: 6 }}>
          <div style={{ opacity: 0.8 }}>Recipient kind</div>
          <div>{summary.recipient_kind}</div>
          <div style={{ opacity: 0.8 }}>Amount</div>
          <div>{summary.amount}</div>
          <div style={{ opacity: 0.8 }}>Fee</div>
          <div>{summary.fee}</div>
          <div style={{ opacity: 0.8 }}>Total spend</div>
          <div>{summary.total_spend}</div>
          <div style={{ opacity: 0.8 }}>Memo</div>
          <div>{summary.memo_present ? 'Yes' : 'No'}</div>
        </div>
      </div>

      <label style={{ display: 'grid', gap: 4, maxWidth: 420 }}>
        <span>Password</span>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.currentTarget.value)}
          disabled={submitting}
        />
      </label>

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
        <button type="button" onClick={submit} disabled={!password || submitting}>
          {submitting ? 'Sending…' : 'Confirm & Send'}
        </button>
        <button type="button" onClick={cancel} disabled={submitting}>
          Cancel
        </button>
      </div>
    </div>
  );
}
