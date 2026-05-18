#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required" >&2
  exit 2
fi

PATTERN='(\\bsentry\\b|@sentry|sentry\\.io|SENTRY_DSN|\\bposthog\\b|POSTHOG|\\bamplitude\\b|\\bsegment\\b|\\bmixpanel\\b|\\bdatadog\\b|\\bnewrelic\\b|\\bbugsnag\\b|\\brollbar\\b|\\bappcenter\\b|\\bcrashlytics\\b)'

TARGETS=(
  "$ROOT/apps/bagz-app-tauri/src"
  "$ROOT/apps/bagz-app-tauri/src-tauri"
  "$ROOT/apps/bagz-app-tauri/package.json"
  "$ROOT/apps/bagz-app-tauri/bun.lock"
  "$ROOT/crates"
  "$ROOT/Cargo.toml"
  "$ROOT/Cargo.lock"
  "$ROOT/.github"
)

EXISTING_TARGETS=()
for target in "${TARGETS[@]}"; do
  if [[ -e "$target" ]]; then
    EXISTING_TARGETS+=("$target")
  fi
done

set +e
rg -n -i "$PATTERN" "${EXISTING_TARGETS[@]}"
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "" >&2
  echo "FAIL: telemetry/crash-reporting integration detected (see matches above)." >&2
  exit 1
fi

if [[ $status -ne 1 ]]; then
  echo "error: rg failed with exit code $status" >&2
  exit 2
fi

echo "PASS: no known telemetry/crash-reporting integrations detected."
