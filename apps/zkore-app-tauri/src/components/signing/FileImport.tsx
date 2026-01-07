import { Buffer } from 'buffer';
import { useState } from 'react';

export function FileImport(props: { onImported: (payloadBase64: string) => void }) {
  const { onImported } = props;
  const [error, setError] = useState<string | null>(null);

  const onChange = async (file: File) => {
    try {
      setError(null);
      const buf = await file.arrayBuffer();
      onImported(Buffer.from(new Uint8Array(buf)).toString('base64'));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to read file');
    }
  };

  return (
    <div style={{ display: 'grid', gap: 8 }}>
      <label style={{ display: 'grid', gap: 4 }}>
        <span>Import signed PCZT (.pczt)</span>
        <input
          type="file"
          accept=".pczt"
          onChange={(e) => {
            const file = e.currentTarget.files?.[0] ?? null;
            e.currentTarget.value = '';
            if (file) void onChange(file);
          }}
        />
      </label>
      {error ? <div style={{ color: 'crimson' }}>{error}</div> : null}
    </div>
  );
}

