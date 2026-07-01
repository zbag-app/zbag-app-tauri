import { expect, test, describe } from 'bun:test';
import { getDisplayableMemos, getMemoDisplayText } from '../utils/memo';
import type * as IPC from '../types/ipc';

describe('getDisplayableMemos', () => {
  test('filters out Empty memos', () => {
    const memos: IPC.MemoInfo[] = [
      { kind: 'Text', content: 'Hello', size_bytes: 5 },
      { kind: 'Empty', content: null, size_bytes: 0 },
      { kind: 'Text', content: 'World', size_bytes: 5 },
    ];
    const result = getDisplayableMemos(memos);
    expect(result).toHaveLength(2);
    expect(result[0].content).toBe('Hello');
    expect(result[1].content).toBe('World');
  });

  test('filters out memos with null content', () => {
    const memos: IPC.MemoInfo[] = [
      { kind: 'Text', content: 'Valid', size_bytes: 5 },
      { kind: 'Text', content: null, size_bytes: 0 },
      { kind: 'Binary', content: null, size_bytes: 512 },
    ];
    const result = getDisplayableMemos(memos);
    expect(result).toHaveLength(1);
    expect(result[0].content).toBe('Valid');
  });

  test('handles Binary memos with placeholder content', () => {
    const memos: IPC.MemoInfo[] = [
      { kind: 'Binary', content: '[binary: 512 bytes]', size_bytes: 512 },
      { kind: 'Text', content: 'Text memo', size_bytes: 9 },
    ];
    const result = getDisplayableMemos(memos);
    expect(result).toHaveLength(2);
    expect(result[0].content).toBe('[binary: 512 bytes]');
    expect(result[1].content).toBe('Text memo');
  });

  test('returns empty array for empty input', () => {
    const result = getDisplayableMemos([]);
    expect(result).toHaveLength(0);
  });

  test('returns empty array when all memos are Empty', () => {
    const memos: IPC.MemoInfo[] = [
      { kind: 'Empty', content: null, size_bytes: 0 },
      { kind: 'Empty', content: null, size_bytes: 0 },
    ];
    const result = getDisplayableMemos(memos);
    expect(result).toHaveLength(0);
  });
});

describe('getMemoDisplayText', () => {
  test('returns single memo content without separator', () => {
    const memos: IPC.MemoInfo[] = [{ kind: 'Text', content: 'Hello', size_bytes: 5 }];
    const result = getMemoDisplayText(memos);
    expect(result).toBe('Hello');
  });

  test('joins multiple memos with separator', () => {
    const memos: IPC.MemoInfo[] = [
      { kind: 'Text', content: 'First', size_bytes: 5 },
      { kind: 'Text', content: 'Second', size_bytes: 6 },
      { kind: 'Text', content: 'Third', size_bytes: 5 },
    ];
    const result = getMemoDisplayText(memos);
    expect(result).toBe('First\n---\nSecond\n---\nThird');
  });

  test('returns empty string for empty array', () => {
    const result = getMemoDisplayText([]);
    expect(result).toBe('');
  });

  test('handles null content gracefully', () => {
    // Although getDisplayableMemos filters these out, getMemoDisplayText
    // should still handle them gracefully
    const memos: IPC.MemoInfo[] = [
      { kind: 'Text', content: null, size_bytes: 0 },
      { kind: 'Text', content: 'Valid', size_bytes: 5 },
    ];
    const result = getMemoDisplayText(memos);
    expect(result).toBe('\n---\nValid');
  });

  test('handles Binary memo placeholder', () => {
    const memos: IPC.MemoInfo[] = [
      { kind: 'Binary', content: '[binary: 256 bytes]', size_bytes: 256 },
    ];
    const result = getMemoDisplayText(memos);
    expect(result).toBe('[binary: 256 bytes]');
  });

  test('preserves memo content with special characters', () => {
    const memos: IPC.MemoInfo[] = [
      { kind: 'Text', content: 'Line1\nLine2', size_bytes: 11 },
      { kind: 'Text', content: 'With "quotes"', size_bytes: 13 },
    ];
    const result = getMemoDisplayText(memos);
    expect(result).toBe('Line1\nLine2\n---\nWith "quotes"');
  });
});
