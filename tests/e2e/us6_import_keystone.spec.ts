// US6 Keystone import e2e test (scaffold).
//
// This repo currently does not wire an automated Tauri e2e harness into `bun test`.
// The intent of this file is to reserve the spec-kit path and document the checks
// required by T127a.
//
// Suggested future approach:
// - Launch the Tauri app in a test mode with a temp HOME / wallets dir
// - Create wallet, complete backup verification, then navigate Settings -> Import Keystone
// - Paste UFVK and import; assert account appears in selector with watch-only badge
// - Switch accounts and verify balances update
// - Attempt to send from watch-only account and assert signing flow is entered (US7)

import { test } from "bun:test";

test.skip("US6 import Keystone UFVK", async () => {
  // TODO: Implement when a Tauri e2e harness is available.
});

