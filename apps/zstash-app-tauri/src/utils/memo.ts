import type * as IPC from '../types/ipc';

/**
 * Filters memos to only include displayable ones (non-Empty with content).
 * Empty memos and memos with null content are excluded.
 */
export function getDisplayableMemos(memos: IPC.MemoInfo[]): IPC.MemoInfo[] {
  return memos.filter((m) => m.kind !== 'Empty' && m.content);
}

/**
 * Joins displayable memos into a single string for display.
 * Multiple memos are separated by "---" on their own line.
 */
export function getMemoDisplayText(displayableMemos: IPC.MemoInfo[]): string {
  return displayableMemos.map((m) => m.content ?? '').join('\n---\n');
}
