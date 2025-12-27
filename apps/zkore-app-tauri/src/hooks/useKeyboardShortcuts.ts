import { useHotkeys } from 'react-hotkeys-hook';

export function useKeyboardShortcuts(keys: string, handler: () => void) {
  useHotkeys(keys, handler, { enableOnFormTags: true });
}
