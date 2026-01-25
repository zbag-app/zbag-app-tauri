import { z } from 'zod';

/**
 * Zod schemas for runtime validation of IPC responses.
 *
 * These schemas validate the structure of responses from the test bridge
 * to catch malformed responses early and provide clear error messages.
 */

export const IpcErrorSchema = z.object({
  code: z.string(),
  message: z.string(),
  details: z.record(z.unknown()).optional(),
});

/**
 * Base schema for all IPC results.
 * All IPC responses are either { ok: T } or { err: IpcError }.
 */
export const BaseIpcResultSchema = z.union([
  z.object({ ok: z.unknown() }),
  z.object({ err: IpcErrorSchema }),
]);

export type IpcError = z.infer<typeof IpcErrorSchema>;
