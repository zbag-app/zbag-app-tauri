#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT/apps/bagz-app-tauri"
TAURI_MANIFEST="$APP_DIR/src-tauri/Cargo.toml"
LOCKFILE="$ROOT/Cargo.lock"
CARGO_HOME_DIR="${CARGO_HOME:-$HOME/.cargo}"

if [[ ! -f "$LOCKFILE" ]]; then
  echo "error: Cargo.lock not found at $LOCKFILE" >&2
  exit 1
fi

if [[ ! -f "$TAURI_MANIFEST" ]]; then
  echo "error: Tauri manifest not found at $TAURI_MANIFEST" >&2
  exit 1
fi

tauri_source_line="$(
  awk '
    $0 == "[[package]]" { in_pkg = 0 }
    $0 ~ /^name = "tauri"$/ { in_pkg = 1 }
    in_pkg && $0 ~ /^source = "git\+https:\/\/github.com\/tauri-apps\/tauri\?rev=/ {
      print $0
      exit
    }
  ' "$LOCKFILE"
)"

if [[ -z "$tauri_source_line" ]]; then
  echo "error: failed to find tauri git source in $LOCKFILE" >&2
  exit 1
fi

tauri_rev="$(sed -E 's/.*#([0-9a-f]+)".*/\1/' <<<"$tauri_source_line")"
if [[ -z "$tauri_rev" ]]; then
  echo "error: failed to parse tauri revision from lockfile source: $tauri_source_line" >&2
  exit 1
fi
tauri_short_rev="${tauri_rev:0:7}"

resolve_manifest_path() {
  local manifest_path=""
  manifest_path="$(find "$CARGO_HOME_DIR/git/checkouts" -type f -path "*/${tauri_short_rev}/crates/tauri-cli/Cargo.toml" 2>/dev/null | head -n 1 || true)"
  if [[ -n "$manifest_path" ]]; then
    echo "$manifest_path"
  fi
}

tauri_cli_manifest_path="${TAURI_CLI_MANIFEST_PATH:-}"

if [[ -z "$tauri_cli_manifest_path" ]]; then
  tauri_cli_manifest_path="$(resolve_manifest_path)"
fi

if [[ -z "$tauri_cli_manifest_path" ]]; then
  echo "info: tauri-cli manifest not found in cargo git checkouts, running cargo fetch..." >&2
  cargo fetch --manifest-path "$TAURI_MANIFEST"
  tauri_cli_manifest_path="$(resolve_manifest_path)"
fi

if [[ -z "$tauri_cli_manifest_path" || ! -f "$tauri_cli_manifest_path" ]]; then
  echo "error: unable to locate tauri-cli manifest for revision $tauri_rev" >&2
  echo "hint: set TAURI_CLI_MANIFEST_PATH explicitly if your cargo checkout layout is custom" >&2
  exit 1
fi

features="${TAURI_FEATURES:-cef-runtime}"
bundles="${TAURI_BUNDLES:-}"
if [[ -z "$bundles" && "$(uname -s)" == "Darwin" ]]; then
  bundles="app,dmg"
fi

build_args=(build --features "$features")
if [[ -n "$bundles" ]]; then
  build_args+=(--bundles "$bundles")
fi
build_args+=("$@")

echo "info: using tauri-cli manifest: $tauri_cli_manifest_path"
echo "info: building with features: $features"
if [[ -n "$bundles" ]]; then
  echo "info: requested bundles: $bundles"
fi

cd "$APP_DIR"
cargo run --manifest-path "$tauri_cli_manifest_path" -- "${build_args[@]}"
