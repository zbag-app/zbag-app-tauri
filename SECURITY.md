# Security Policy

zbag is a privacy-first Zcash wallet. In this Tauri/CEF release track, the core
trust boundary is that wallet secrets remain in the Rust backend. The React UI
and CEF renderer should only handle opaque handles, explicit user-entered
fields, and serializable non-secret data except for narrow, user-approved
mnemonic flows.

## Reporting a vulnerability

Please report security issues privately. Do not open a public issue, pull
request, or discussion for a suspected vulnerability.

- Email: `security@zbag.app`
- If you need encrypted transport, request our PGP key at that address before
  sending details.

Please include enough information to reproduce and assess the issue: affected
version or commit, platform, a description of the impact, and step-by-step
reproduction or a proof of concept where possible.

We will acknowledge your report, work with you to understand and validate the
issue, keep you informed as we remediate, and credit you in the release notes
if you wish. We follow coordinated disclosure and ask that you give us a
reasonable opportunity to ship a fix before public disclosure.

## Safe harbor

We will not pursue or support legal action against researchers who, in good
faith and in accordance with this policy:

- make a good-faith effort to avoid privacy violations, data destruction, and
  interruption or degradation of services;
- only interact with accounts and wallets they own or have explicit permission
  to test;
- do not access, modify, or exfiltrate other users' funds or data; and
- report promptly and give us a reasonable time to remediate before disclosure.

Activity conducted consistently with this policy is considered authorized. If
in doubt, ask us at the address above before proceeding.

## In scope

The following classes of issue are the highest priority:

- Seed, mnemonic, viewing key, or spending-key extraction from the Rust backend.
- Mnemonic, seed, key, memo, password, or reauth-token leakage to logs or
  renderer state outside intentional user flows.
- Tor fail-open or clearnet leak when Tor is required.
- CEF renderer networking that bypasses the app's Rust networking controls.
- Signing-payload tampering in the Keystone / PCZT flow.
- Privacy downgrades that are not explicit and user-acknowledged.

Also in scope: transparent funds being spendable without clear shielding
guidance, memory that should be zeroized but is not, and regressions in CEF
hardening guardrails.

## Out of scope

- Vulnerabilities in third-party dependencies that do not affect zbag.
- Issues requiring a fully compromised host, malicious OS, debugger, hostile
  kernel, or physical access with the wallet already unlocked.
- Social engineering, phishing, or attacks against the user rather than the
  software.

## No bounty program

zbag does not currently operate a paid bug-bounty program, and no monetary
reward amounts are stated or implied. We are grateful for responsible reports
and will credit reporters who wish to be named.

The licensor and security contact for zbag is **Quellkern e.U.** (Reza Shokri).
