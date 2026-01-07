// US7 Keystone air-gapped signing e2e test (scaffold).
//
// This repo currently does not wire an automated Tauri e2e harness into `bun test`.
// The intent of this file is to reserve the spec-kit path and document the checks
// required by T141a/T141b/T141c.
//
// Suggested future approach:
// - Create wallet, complete backup verification, and import a Keystone UFVK (US6)
// - Attempt to send from HardwareSigner account -> Signing screen
// - Validate animated QR renders and file export name is generic (`zkore-unsigned.pczt`)
// - Import a signed PCZT via QR/file and broadcast; validate Activity entry
// - Exercise transparent-recipient privacy acknowledgement and memo blocking
// - Exercise malformed payloads and verify stable, user-safe errors

import { test } from "bun:test";

test.skip("US7 Keystone air-gapped signing", async () => {
  // TODO: Implement when a Tauri e2e harness is available.
});
