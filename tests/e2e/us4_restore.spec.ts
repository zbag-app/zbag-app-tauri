// US4 restore e2e test (scaffold).
//
// This repo currently does not wire an automated Tauri e2e harness into `bun test`.
// The intent of this file is to reserve the spec-kit path and document the checks
// required by T115a.
//
// Suggested future approach:
// - Launch the Tauri app in a test mode with a temp HOME / wallets dir
// - Drive restore screens (RestoreWallet -> RestoreBirthday -> Home)
// - Assert sync progress phases + ETA appear and update
// - Assert restored wallets show backup_status === "Complete" immediately after restore
// - Assert spending is not blocked by BACKUP_REQUIRED (may still fail for insufficient spendable)

import { test } from "bun:test";

test.skip("US4 restore flow", async () => {
  // TODO: Implement when a Tauri e2e harness is available.
});

