#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_TAURI="$ROOT/apps/zbag-app-tauri/src-tauri"
LIB_RS="$SRC_TAURI/src/lib.rs"

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required" >&2
  exit 2
fi

extract_region() {
  local begin_marker="$1"
  local end_marker="$2"
  local region

  region="$(awk "/$begin_marker/,/$end_marker/" "$LIB_RS")"
  if [[ -z "$region" ]] || ! grep -Fq "$begin_marker" <<<"$region" || ! grep -Fq "$end_marker" <<<"$region"; then
    echo "error: missing CEF hardening marker region: $begin_marker .. $end_marker" >&2
    exit 2
  fi

  printf '%s\n' "$region"
}

extract_const_concat() {
  local const_name="$1"
  local region

  region="$(awk "/const ${const_name}:.*concat!/,/);/" "$LIB_RS")"
  if [[ -z "$region" ]] || ! grep -Fq "$const_name" <<<"$region" || ! grep -Fq "concat!(" <<<"$region"; then
    echo "error: missing CEF hardening const concat region: $const_name" >&2
    exit 2
  fi

  printf '%s\n' "$region"
}

extract_function() {
  local function_name="$1"
  local region

  region="$(awk "/fn ${function_name}\\(/,/^}/" "$LIB_RS")"
  if [[ -z "$region" ]] || ! grep -Fq "fn $function_name" <<<"$region"; then
    echo "error: missing CEF hardening function region: $function_name" >&2
    exit 2
  fi

  printf '%s\n' "$region"
}

require_in_region() {
  local label="$1"
  local region="$2"
  shift 2

  local missing=0
  local literal
  for literal in "$@"; do
    if ! grep -Fq -- "$literal" <<<"$region"; then
      echo "FAIL: missing CEF $label literal: $literal" >&2
      missing=1
    fi
  done

  return "$missing"
}

require_absent_in_region() {
  local label="$1"
  local region="$2"
  shift 2

  local found=0
  local literal
  for literal in "$@"; do
    if grep -Fq -- "$literal" <<<"$region"; then
      echo "FAIL: forbidden CEF $label literal present: $literal" >&2
      found=1
    fi
  done

  return "$found"
}

REQUIRED_GENERAL=(
  "root_cache_path(&cef_cache_path)"
  "purge_legacy_cef_cache"
  "purge_stale_temp_cef_caches"
  "enforce_cef_browser_policy"
  "failed to remove temp CEF cache"
)

REQUIRED_SWITCHES=(
  "disable-background-networking"
  "disable-breakpad"
  "disable-component-extensions-with-background-pages"
  "disable-component-update"
  "disable-default-apps"
  "disable-domain-reliability"
  "disable-extensions"
  "disable-field-trial-config"
  "disable-notifications"
  "disable-print-preview"
  "disable-save-password-bubble"
  "disable-speech-api"
  "disable-sync"
  "disable-sync-invalidation-optimizations"
  "incognito"
  "metrics-recording-only"
  "no-default-browser-check"
  "no-first-run"
  "no-pings"
)

REQUIRED_VALUED_ARGS=(
  "disable-features"
  "dns-over-https-mode"
  "dns-over-https-templates"
  "host-resolver-rules"
  "webrtc-ip-handling-policy"
)

REQUIRED_DISABLED_FEATURES=(
  "AutofillActorMode"
  "AutofillServerCommunication"
  "AsyncDns"
  "DnsOverHttpsUpgrade"
  "EnableMediaRouter"
  "GlicActorUi"
  "LensOverlay"
  "LiveTranslate"
  "MediaRouter"
  "OptimizationGuideModelExecution"
  "OptimizationGuideOnDeviceModel"
  "OptimizationHints"
  "PrivacySandboxSettings4"
  "Translate"
  "UseDnsHttpsSvcb"
)

REQUIRED_HOST_RESOLVER_EXCLUDES=(
  "MAP * 0.0.0.0"
  "EXCLUDE localhost"
  "EXCLUDE 127.0.0.1"
  "EXCLUDE ::1"
  "EXCLUDE *.localhost"
  "EXCLUDE ipc.localhost"
  "EXCLUDE tauri.localhost"
)

REQUIRED_PREFS=(
  "safebrowsing"
  "dns_over_https"
  "network_prediction_options"
  "search"
  "signin"
  "spellcheck"
  "translate"
  "autofill"
  "credentials_enable_service"
)

FORBIDDEN_LITERALS=(
  "--enable-features="
  "--proxy-server="
  "--proxy-pac-url="
  "dns-over-https-mode=automatic"
  "dns-over-https-mode=secure"
  "googleapis.com"
  "gvt1.com"
  "clients2.google.com"
  "cloudflare-dns.com"
  "doh.opendns.com"
  "youtube.com"
  "gstatic.com"
)

FORBIDDEN_IN_RUNTIME_ARGS=(
  "enable-features"
  "proxy-server"
  "proxy-pac-url"
  "remote-debugging-port"
  "remote-debugging-pipe"
)

missing=0

require_in_region "general" "$(cat "$LIB_RS")" "${REQUIRED_GENERAL[@]}" || missing=1
require_in_region "switch" \
  "$(extract_region "CEF_HARDENING_SWITCHES_BEGIN" "CEF_HARDENING_SWITCHES_END")" \
  "${REQUIRED_SWITCHES[@]}" || missing=1
require_in_region "valued arg" \
  "$(extract_region "CEF_HARDENING_VALUED_ARGS_BEGIN" "CEF_HARDENING_VALUED_ARGS_END")" \
  "${REQUIRED_VALUED_ARGS[@]}" || missing=1
require_in_region "disabled feature" \
  "$(extract_const_concat "CEF_DISABLED_FEATURES")" \
  "${REQUIRED_DISABLED_FEATURES[@]}" || missing=1
require_in_region "host resolver rule" \
  "$(extract_const_concat "CEF_HOST_RESOLVER_RULES")" \
  "${REQUIRED_HOST_RESOLVER_EXCLUDES[@]}" || missing=1
require_in_region "browser preference" \
  "$(extract_region "CEF_HARDENING_PREFS_BEGIN" "CEF_HARDENING_PREFS_END")" \
  "${REQUIRED_PREFS[@]}" || missing=1
require_absent_in_region "runtime args" \
  "$(extract_function "cef_runtime_args")" \
  "${FORBIDDEN_IN_RUNTIME_ARGS[@]}" || missing=1

for literal in "${FORBIDDEN_LITERALS[@]}"; do
  if rg -n -F -- "$literal" "$SRC_TAURI"; then
    echo "FAIL: forbidden CEF network-hardening literal present under src-tauri: $literal" >&2
    missing=1
  fi
done

if [[ "$missing" -ne 0 ]]; then
  exit 1
fi

echo "PASS: CEF network hardening guardrails are present."
