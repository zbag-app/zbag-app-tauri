export function ServerSecurityWarning() {
  return (
    <div
      role="note"
      aria-label="Custom server security warning"
      style={{
        border: '1px solid #f59e0b',
        background: '#fffbeb',
        borderRadius: 12,
        padding: 12,
        display: 'grid',
        gap: 8,
      }}
    >
      <strong>Security warning</strong>
      <div style={{ fontSize: 14, opacity: 0.9 }}>
        Custom lightwalletd servers can see your IP address and may be able to infer wallet activity
        patterns. Prefer trusted defaults, and use Tor for network privacy.
      </div>
    </div>
  );
}

