// US1 onboarding e2e test (scaffold).
//
// This repo currently does not wire an automated Tauri e2e harness into `bun test`.
// The intent of this file is to reserve the spec-kit path and document the checks
// required by T085a.
//
// Suggested future approach:
// - Launch the Tauri app in a test mode with a temp HOME / wallets dir
// - Drive onboarding screens (CreateWallet -> SeedDisplay -> BackupChallenge -> Home)
// - Assert backup gating and lock/unlock behavior end-to-end
// - Record CreateWallet duration in release builds (not CI-gated)

import { test } from "bun:test";

test.skip("US1 onboarding flow", async () => {
  // TODO: Implement when a Tauri e2e harness is available.
});

