import { expect, test } from 'bun:test';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { MenuEvents } from '../constants/menuEvents';

const appRoot = path.resolve(import.meta.dir, '..', '..');

async function readRustMenuRs(): Promise<string> {
  return readFile(path.join(appRoot, 'src-tauri', 'src', 'menu.rs'), 'utf8');
}

function parseRustMenuEvents(source: string): Record<string, string> {
  const events: Record<string, string> = {};
  const re = /pub const ([A-Z0-9_]+): &str = "([^"]+)";/g;

  for (const match of source.matchAll(re)) {
    const key = match[1];
    const value = match[2];
    if (events[key] != null) {
      throw new Error(`duplicate Rust menu event const: ${key}`);
    }
    events[key] = value;
  }

  return events;
}

test('menu event constants are in sync with Rust backend', async () => {
  const rustSource = await readRustMenuRs();
  const rustEvents = parseRustMenuEvents(rustSource);

  const tsKeys = Object.keys(MenuEvents).sort();
  const rustKeys = Object.keys(rustEvents).sort();
  expect(rustKeys).toEqual(tsKeys);

  for (const key of tsKeys) {
    expect(rustEvents[key]).toBe(MenuEvents[key as keyof typeof MenuEvents]);
  }
});

