# Third-Party Notices

zbag is built on open-source software and on wallet code carried forward from
earlier projects by the same author. zbag itself is licensed under
FSL-1.1-ALv2. The notices below cover third-party components and historical
provenance; they do not narrow or override the licenses those parties grant.

## Dependency License Families

The exact dependency graph is defined by [`Cargo.lock`](./Cargo.lock) and the
Tauri frontend lockfile. Release packaging should refresh a machine-readable
dependency license inventory before publication.

### Rust workspace

- **Zcash protocol crates** (`zcash_client_backend`, `zcash_client_sqlite`,
  `zcash_primitives`, `zcash_proofs`, `zcash_protocol`, `orchard`, `zip32`, and
  related librustzcash crates): generally **MIT OR Apache-2.0**.
- **Async runtime and networking** (`tokio`, `tonic`, `hyper`, `tower`,
  `rustls`, `ring`, `webpki-roots`): generally **MIT**, **Apache-2.0**, or
  compatible dual licensing; `ring` carries additional ISC-style and
  OpenSSL-derived notices.
- **Serialization and utilities** (`serde`, `serde_json`, `prost`, `bytes`,
  `tracing`, `anyhow`, `thiserror`): generally **MIT OR Apache-2.0**.
- **Cryptography and secret handling** (`argon2`, `chacha20poly1305`,
  `zeroize`, `bip39`, `secrecy`): generally **MIT OR Apache-2.0**.
- **Storage** (`rusqlite` with bundled SQLCipher): `rusqlite` is **MIT**;
  SQLCipher is distributed under a BSD-style license by Zetetic LLC.
- **Tor** (`arti` and the `arti-client` family, consumed through Zcash client
  dependencies): generally **MIT OR Apache-2.0**.
- **Desktop shell** (`tauri`, `tauri-build`, and related crates): generally
  **MIT OR Apache-2.0**.

### Tauri/React frontend

- **React**, **Vite**, **TypeScript**, **Tailwind CSS**, **Radix UI**,
  **lucide-react**, **TanStack Query**, **Zod**, and supporting npm packages:
  predominantly **MIT**.
- **Tauri JavaScript APIs** (`@tauri-apps/api`, `@tauri-apps/cli`,
  `@tauri-apps/plugin-opener`): generally **MIT OR Apache-2.0**.
- **Keystone and QR dependencies** (`@keystonehq/*`, `@zxing/*`,
  `qrcode.react`): generally permissive licenses such as **MIT** or
  **Apache-2.0**.

### Bundled fonts

- **Syne**: SIL Open Font License 1.1 (**OFL-1.1**).
- **JetBrains Mono**: SIL Open Font License 1.1 (**OFL-1.1**).

### CEF runtime

This release track uses Tauri's experimental CEF runtime path. Chromium
Embedded Framework and Chromium include multiple open-source components under
BSD-style, MIT, Apache-2.0, and other permissive licenses. Release artifacts
that bundle CEF should include the upstream CEF/Chromium notices supplied with
the runtime payload.

## Historical Lineage

zbag's Rust wallet core and desktop application history were carried forward
from earlier projects by the same author:

- **zstash** - the original line, licensed **MIT**.
- **zbag / zbag** - the successor line, licensed under the
  **PolyForm Shield License 1.0.0**.

The following statements record the provenance and licensing transition:

1. **Authorship.** The reused wallet core, desktop shell, and CLI were authored
   by Reza Shokri (GitHub: devdotbo), except for third-party dependencies and
   explicitly attributed third-party code.

2. **Historical licenses are preserved.** Commits made under zstash (MIT) and
   zbag / zbag (PolyForm Shield 1.0.0) remain available under the licenses they
   were originally released under. Nothing in zbag's relicensing retroactively
   changes the terms under which those historical versions were distributed.
   The git history is intentionally preserved so this lineage stays visible.

3. **FSL applies going forward.** From the zbag line onward, the Software is
   made available under **FSL-1.1-ALv2** by **Quellkern e.U.** (Reza Shokri).
   Each released zbag version additionally becomes available under the Apache
   License, Version 2.0 on its second anniversary, per the Grant of Future
   License in [`LICENSE`](./LICENSE).

4. **Third-party dependencies remain under their own licenses.** The
   relicensing does not alter the licenses of upstream crates, npm packages,
   fonts, Tauri, CEF, Chromium, Zcash dependencies, or other third-party
   components.
