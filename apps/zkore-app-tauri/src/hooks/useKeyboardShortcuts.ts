import { useHotkeys } from 'react-hotkeys-hook';

export function useKeyboardShortcuts(keys: string, handler: () => void, enabled = true) {
  useHotkeys(keys, handler, { enableOnFormTags: true, enabled });
}
