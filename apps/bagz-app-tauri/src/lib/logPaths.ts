import type * as IPC from '../types/ipc';

export type LogLocationLike = Pick<IPC.GetLogLocationResponse, 'current_log_file' | 'log_directory'>;

/**
 * Returns the best path to reveal for log locations, preferring the active log file.
 * Empty/whitespace-only values are treated as missing.
 */
export function resolveLogRevealPath(location: LogLocationLike): string | null {
  const currentLogFile = location.current_log_file.trim();
  if (currentLogFile.length > 0) return currentLogFile;

  const logDirectory = location.log_directory.trim();
  if (logDirectory.length > 0) return logDirectory;

  return null;
}
