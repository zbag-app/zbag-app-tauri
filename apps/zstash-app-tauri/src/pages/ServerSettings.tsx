import { useEffect, useMemo, useState } from 'react';
import { Server, Plus, PlayCircle, Star } from 'lucide-react';
import type * as IPC from '../types/ipc';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Label } from '../components/ui/label';
import { Badge } from '../components/ui/badge';
import { ServerSecurityWarning } from '../components/settings/ServerSecurityWarning';
import { addServer, listServers, setDefaultServer, testServer } from '../services/ipc';

function formatMaybeMs(ms: number | null | undefined): string {
  if (!ms) return '-';
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
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <Server className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Servers</h1>
      </div>

      <ServerSecurityWarning />

      {error && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
          {error}
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle className="text-lg flex items-center gap-2">
            Active Network
            <Badge variant={wallet.network === 'Mainnet' ? 'success' : 'warning'}>
              {wallet.network}
            </Badge>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            Showing servers for this network only.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Add Server</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="serverName">Name</Label>
            <Input
              id="serverName"
              value={name}
              onChange={(e) => setName(e.currentTarget.value)}
              disabled={loading}
              placeholder="My Server"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="grpcUrl">gRPC URL</Label>
            <Input
              id="grpcUrl"
              value={grpcUrl}
              onChange={(e) => setGrpcUrl(e.currentTarget.value)}
              disabled={loading}
              placeholder="https://lwd.example.com:443"
            />
          </div>
          <Button onClick={submitAdd} disabled={loading || !name.trim() || !grpcUrl.trim()}>
            <Plus className="h-4 w-4" />
            {loading ? 'Adding...' : 'Add'}
          </Button>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Servers ({visibleServers.length})</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          {visibleServers.length === 0 ? (
            <p className="text-muted-foreground">No servers configured for this network.</p>
          ) : (
            visibleServers.map((s) => {
              const last = testResult[s.id];
              return (
                <div
                  key={s.id}
                  className="rounded-lg border border-border p-4 space-y-3"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="space-y-1 flex-1">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="font-semibold">{s.name}</span>
                        <Badge variant={s.network === 'Mainnet' ? 'success' : 'warning'}>
                          {s.network}
                        </Badge>
                        {s.is_default && (
                          <Badge variant="default" className="gap-1">
                            <Star className="h-3 w-3" />
                            Default
                          </Badge>
                        )}
                      </div>
                      <code className="text-xs text-muted-foreground break-all font-mono">
                        {s.grpc_url}
                      </code>
                    </div>
                    <div className="flex gap-2 shrink-0">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => runTest(s.id)}
                        disabled={loading}
                      >
                        <PlayCircle className="h-4 w-4" />
                        Test
                      </Button>
                      {!s.is_default && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => setDefault(s.id)}
                          disabled={loading}
                        >
                          <Star className="h-4 w-4" />
                          Set default
                        </Button>
                      )}
                    </div>
                  </div>

                  <div className="text-xs text-muted-foreground">
                    Last success: {formatMaybeMs(s.last_success_at)}
                  </div>

                  {last && (
                    <div className={`text-xs ${last.success ? 'text-success' : 'text-destructive'}`}>
                      Test result: {last.success ? 'OK' : 'FAIL'}
                      {last.latency_ms !== null && ` (${last.latency_ms}ms)`}
                      {last.error && ` - ${last.error}`}
                    </div>
                  )}
                </div>
              );
            })
          )}
        </CardContent>
      </Card>
    </div>
  );
}
