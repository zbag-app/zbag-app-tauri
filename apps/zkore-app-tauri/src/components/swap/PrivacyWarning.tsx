export function PrivacyWarning(props: {
  acknowledged: boolean;
  onAcknowledgedChange: (next: boolean) => void;
}) {
  const { acknowledged, onAcknowledgedChange } = props;

  return (
    <div style={{ padding: 12, border: '1px solid #c0841a', borderRadius: 8 }}>
      <strong>Privacy warning</strong>
      <div style={{ marginTop: 6, fontSize: 13, opacity: 0.9 }}>
        This swap may require transparent Zcash interaction (for example: sending to a transparent address
        or generating a temporary transparent refund address). Transparent interactions can reduce privacy
        by making amounts and addresses visible on-chain.
      </div>
      <label style={{ display: 'flex', gap: 8, alignItems: 'center', marginTop: 10 }}>
        <input
          type="checkbox"
          checked={acknowledged}
          onChange={(e) => onAcknowledgedChange(e.currentTarget.checked)}
        />
        <span style={{ fontSize: 13 }}>I understand and want to continue.</span>
      </label>
    </div>
  );
}

