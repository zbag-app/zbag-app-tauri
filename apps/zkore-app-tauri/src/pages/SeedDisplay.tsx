import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

export function SeedDisplay(props: { seedPhrase: string[]; onCleared: () => void }) {
  const { seedPhrase, onCleared } = props;
  const navigate = useNavigate();

  const [words, setWords] = useState<string[]>(seedPhrase);

  useEffect(() => {
    return () => {
      setWords([]);
      onCleared();
    };
  }, [onCleared]);

  const wordRows = useMemo(() => {
    const rows: Array<{ index: number; word: string }> = [];
    for (let i = 0; i < words.length; i += 1) {
      rows.push({ index: i + 1, word: words[i] });
    }
    return rows;
  }, [words]);

  if (words.length !== 24) {
    return (
      <div style={{ display: 'grid', gap: 12, padding: 16 }}>
        <h1>Seed phrase</h1>
        <div>No seed phrase in memory.</div>
        <button type="button" onClick={() => navigate('/')}>
          Go home
        </button>
      </div>
    );
  }

  return (
    <div style={{ display: 'grid', gap: 12, padding: 16, maxWidth: 720 }}>
      <h1>Write down your seed phrase</h1>
      <p>
        This is the only way to recover your wallet. Do not screenshot, copy/paste, or store it in
        cloud notes.
      </p>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
          gap: 8,
          userSelect: 'none',
        }}
      >
        {wordRows.map(({ index, word }) => (
          <div
            key={index}
            style={{
              display: 'flex',
              gap: 8,
              padding: 8,
              border: '1px solid #ddd',
              borderRadius: 8,
              background: '#fafafa',
            }}
          >
            <span style={{ width: 28, opacity: 0.7 }}>{index}.</span>
            <strong>{word}</strong>
          </div>
        ))}
      </div>

      <button type="button" onClick={() => navigate('/backup')}>
        Continue to backup verification
      </button>
    </div>
  );
}

