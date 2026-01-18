export function PrivacyWarning(props: {
  acknowledged: boolean;
  onAcknowledgedChange: (next: boolean) => void;
}) {
  const { acknowledged, onAcknowledgedChange } = props;

  return (
    <div className="p-3 rounded-none border border-warning/50 bg-warning/10">
      <strong className="text-warning">Privacy warning</strong>
      <div className="mt-1.5 text-sm text-muted-foreground">
        This swap may require transparent Zcash interaction (for example: sending to a transparent address
        or generating a temporary transparent refund address). Transparent interactions can reduce privacy
        by making amounts and addresses visible on-chain.
      </div>
      <label className="flex gap-2 items-center mt-2.5 cursor-pointer">
        <input
          type="checkbox"
          checked={acknowledged}
          onChange={(e) => onAcknowledgedChange(e.currentTarget.checked)}
          className="accent-primary"
        />
        <span className="text-sm">I understand and want to continue.</span>
      </label>
    </div>
  );
}
