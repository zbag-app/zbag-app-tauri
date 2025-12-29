import { useEffect, useMemo, useState } from 'react';
import type * as IPC from '../types/ipc';
import { NetworkBadge } from '../components/common/NetworkBadge';
import { ServerSecurityWarning } from '../components/settings/ServerSecurityWarning';
import { addServer, listServers, setDefaultServer, testServer } from '../services/ipc';

function formatMaybeMs(ms: number | null | undefined): string {
  if (!ms) return '—';
  try {
    return new Date(ms).toLocaleString();
  } catch {
    return String(ms);
  }
}

export function ServerSettings(props: { wallet: IPC.WalletInfo }) {
  const { wallet } = props;

  const [servers, setServers] = useState<IPC.ServerInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [name, setName] = useState('');
  const [grpcUrl, setGrpcUrl] = useState('');

  const [testResult, setTestResult] = useState<Record<string, IPC.TestServerResponse>>({});

  const visibleServers = useMemo(
    () => servers.filter((s) => s.network === wallet.network),
    [servers, wallet.network]
  );

  const refresh = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await listServers();
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setServers(res.ok.servers);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wallet.network]);

  const submitAdd = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await addServer({ name: name.trim(), grpc_url: grpcUrl.trim() });
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setName('');
      setGrpcUrl('');
      await refresh();
    } finally {
      setLoading(false);
    }
  };

  const setDefault = async (serverId: string) => {
    setLoading(true);
    setError(null);
    try {
      const res = await setDefaultServer({ server_id: serverId });
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      await refresh();
    } finally {
      setLoading(false);
    }
  };

  const runTest = async (serverId: string) => {
    setLoading(true);
    setError(null);
    try {
      const res = await testServer({ server_id: serverId });
      if ('err' in res) {
        setError(res.err.message);
        return;
      }
      setTestResult((prev) => ({ ...prev, [serverId]: res.ok }));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ display: 'grid', gap: 14 }}>
      <h1 style={{ margin: 0 }}>Servers</h1>

      <ServerSecurityWarning />

      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Active network</h2>
        <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
          <NetworkBadge network={wallet.network} />
          <span style={{ fontSize: 12, opacity: 0.8 }}>Showing servers for this network only.</span>
        </div>
      </section>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Add server</h2>
        <label style={{ display: 'grid', gap: 4, maxWidth: 640 }}>
          <span>Name</span>
          <input value={name} onChange={(e) => setName(e.currentTarget.value)} disabled={loading} />
        </label>
        <label style={{ display: 'grid', gap: 4, maxWidth: 640 }}>
          <span>gRPC URL</span>
          <input
            value={grpcUrl}
            onChange={(e) => setGrpcUrl(e.currentTarget.value)}
            disabled={loading}
            placeholder="https://lwd.example.com:443"
          />
        </label>
        <button type="button" onClick={submitAdd} disabled={loading || !name.trim() || !grpcUrl.trim()}>
          {loading ? 'Adding…' : 'Add'}
        </button>
      </section>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ margin: 0 }}>Servers ({visibleServers.length})</h2>
        {visibleServers.length === 0 ? <div>No servers configured for this network.</div> : null}
        {visibleServers.map((s) => {
          const last = testResult[s.id];
          return (
            <div
              key={s.id}
              style={{ border: '1px solid #e5e7eb', borderRadius: 12, padding: 12, display: 'grid', gap: 8 }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
                <div style={{ display: 'grid', gap: 2 }}>
                  <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
                    <strong>{s.name}</strong>
                    <NetworkBadge network={s.network} />
                    {s.is_default ? (
                      <span
                        style={{
                          fontSize: 12,
                          padding: '2px 8px',
                          borderRadius: 999,
                          background: '#e0f2fe',
                          border: '1px solid #38bdf8',
                        }}
                      >
                        Default
                      </span>
                    ) : null}
                  </div>
                  <code style={{ fontSize: 12, opacity: 0.9, wordBreak: 'break-all' }}>{s.grpc_url}</code>
                </div>

                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
                  <button type="button" onClick={() => runTest(s.id)} disabled={loading}>
                    Test
                  </button>
                  {!s.is_default ? (
                    <button type="button" onClick={() => setDefault(s.id)} disabled={loading}>
                      Set default
                    </button>
                  ) : null}
                </div>
              </div>

              <div style={{ fontSize: 12, opacity: 0.8 }}>
                Last success: {formatMaybeMs(s.last_success_at)}
              </div>

              {last ? (
                <div style={{ fontSize: 12, opacity: 0.85 }}>
                  Test result: {last.success ? 'OK' : 'FAIL'}
                  {last.latency_ms !== null ? ` (${last.latency_ms}ms)` : ''}
                  {last.error ? ` — ${last.error}` : ''}
                </div>
              ) : null}
            </div>
          );
        })}
      </section>
    </div>
  );
}

