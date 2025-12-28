import { useMemo } from 'react';
import type * as IPC from '../../types/ipc';

export function SigningVerify(props: { summary: IPC.SigningSummary }) {
  const { summary } = props;

  const warning = useMemo(() => {
    if (summary.recipient_kind !== 'Transparent') return null;
    return 'Transparent recipients reduce privacy. Memos are not supported.';
  }, [summary.recipient_kind]);

  return (
    <div style={{ display: 'grid', gap: 12, maxWidth: 760 }}>
      <h2>Verify transaction</h2>

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
          <div style={{ opacity: 0.8 }}>Memo</div>
          <div>{summary.memo_present ? 'Yes' : 'No'}</div>
        </div>
      </div>
    </div>
  );
}

