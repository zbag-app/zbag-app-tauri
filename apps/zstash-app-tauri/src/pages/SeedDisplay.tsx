import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Key, Home } from 'lucide-react';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Button } from '../components/ui/button';

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
      <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
            <Key className="h-5 w-5 text-primary" />
          </div>
          <h1 className="text-2xl font-bold">Seed Phrase</h1>
        </div>

        <Card>
          <CardContent className="pt-6">
            <p className="text-muted-foreground">No seed phrase in memory.</p>
            <Button variant="outline" onClick={() => navigate('/')} className="mt-4">
              <Home className="h-4 w-4" />
              Go home
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="space-y-6 animate-[fade-in-up_0.4s_ease-out]">
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10">
          <Key className="h-5 w-5 text-primary" />
        </div>
        <h1 className="text-2xl font-bold">Write Down Your Seed Phrase</h1>
      </div>

      <Card className="border-warning/50 bg-warning/5">
        <CardContent className="pt-6">
          <p className="text-sm text-muted-foreground">
            This is the only way to recover your wallet. Do not screenshot, copy/paste, or store it in
            cloud notes.
          </p>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Your 24-Word Seed Phrase</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-3 gap-2 select-none">
            {wordRows.map(({ index, word }) => (
              <div
                key={index}
                className="flex items-center gap-2 rounded-none border border-border bg-muted/50 px-3 py-2"
              >
                <span className="w-6 text-sm text-muted-foreground">{index}.</span>
                <span className="font-mono font-semibold">{word}</span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Button onClick={() => navigate('/backup')} className="w-full">
        Continue to backup verification
      </Button>
    </div>
  );
}
