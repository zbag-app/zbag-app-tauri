import { useMemo } from 'react';
import type * as IPC from '../../types/ipc';
import { formatZatoshisToZec } from '../../utils/zec';

export function SigningVerify(props: { summary: IPC.SigningSummary }) {
  const { summary } = props;

  const warning = useMemo(() => {
    if (summary.recipient_kind !== 'Transparent') return null;
    return 'Transparent recipients reduce privacy. Memos are not supported.';
  }, [summary.recipient_kind]);

  return (
    <div className="grid gap-3 max-w-3xl">
      <h2 className="text-lg font-semibold">Verify transaction</h2>

      {warning ? (
        <div className="p-3 rounded-lg border border-warning/50 bg-warning/10">
          <strong className="text-warning">Privacy warning</strong>
          <div className="mt-1 text-sm text-muted-foreground">{warning}</div>
        </div>
      ) : null}

      <div className="grid gap-2">
        <div className="grid gap-1">
          <div className="text-sm text-muted-foreground">Recipient</div>
          <code className="text-sm break-all">{summary.recipient}</code>
        </div>

        <div className="grid grid-cols-[140px_1fr] gap-1.5 text-sm">
          <div className="text-muted-foreground">Recipient kind</div>
          <div>{summary.recipient_kind}</div>
          <div className="text-muted-foreground">Amount (ZEC)</div>
          <div>{formatZatoshisToZec(summary.amount)}</div>
          <div className="text-muted-foreground">Fee (ZEC)</div>
          <div>{formatZatoshisToZec(summary.fee)}</div>
          <div className="text-muted-foreground">Memo</div>
          <div>{summary.memo_present ? 'Yes' : 'No'}</div>
        </div>
      </div>
    </div>
  );
}
