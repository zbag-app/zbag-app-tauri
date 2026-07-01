# Contributing to zbag

Thank you for your interest in zbag, a privacy-first Zcash wallet. This
release track keeps the current Rust + Tauri/CEF + React desktop application
buildable while the main product line migrates to Flutter.

## Licensing and the DCO

zbag is licensed under **FSL-1.1-ALv2**. See [`LICENSE`](./LICENSE) for the
full text.

Contributions are **inbound = outbound**: when you contribute, you license your
contribution under the same FSL-1.1-ALv2 terms that cover the project, including
the same automatic Apache-2.0 future grant.

We use the **Developer Certificate of Origin (DCO)**, not a CLA. To sign off,
certify the [DCO](https://developercertificate.org/) by adding a
`Signed-off-by` line to every commit:

```text
Signed-off-by: Your Name <you@example.com>
```

`git commit -s` adds this automatically. The name and email should be your real
identity and match your commit author.

## Development Setup

Use the `just` recipes as the stable developer interface:

```bash
just build
just test
just fmt
just clippy
just app-build
just verify
```

The legacy Makefile remains available for lower-level targets while this Tauri
release track is maintained.

## Version Control

This project uses standard **git**.

```bash
git status
git diff
git switch -c fix/description
git commit -s -m "fix: description"
git pull --rebase
```

Use `--force-with-lease` rather than `--force` when rewriting a branch you have
already pushed.

## Commit Messages

Follow the existing Conventional Commit patterns:

- `US<N>: ...` - user-story work
- `docs: ...` - documentation
- `chore: ...` - maintenance
- `fix: ...` - bug fixes
- `feat: ...` - new features

Every new commit must carry a DCO sign-off.

## Code Quality

Before submitting, run:

```bash
just verify
```

For targeted work, use the narrower recipes first:

```bash
just fmt
just clippy
just test
just app-build
```

## Security and Conduct

- Never commit secrets, keys, wallets, or private `.env` files.
- Do not report security vulnerabilities through public issues or PRs; follow
  [`SECURITY.md`](./SECURITY.md).
- Preserve the project guardrails: wallet secrets stay in Rust, shielded funds
  are preferred, Tor fail-closed behavior must not regress, and logs must stay
  redacted.

See [`AGENTS.md`](./AGENTS.md) for detailed contributor and agent guidelines.
