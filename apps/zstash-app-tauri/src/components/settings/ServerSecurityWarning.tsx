export function ServerSecurityWarning() {
  return (
    <div
      role="note"
      aria-label="Custom server security warning"
      className="rounded-xl border border-warning/50 bg-warning/10 p-3 grid gap-2"
    >
      <strong className="text-warning">Security warning</strong>
      <div className="text-sm text-muted-foreground">
        Custom lightwalletd servers can see your IP address and may be able to infer wallet activity
        patterns. Prefer trusted defaults, and use Tor for network privacy.
      </div>
    </div>
  );
}
