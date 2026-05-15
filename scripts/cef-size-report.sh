#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

TOP_N=20
CEF_DIR="${CEF_DIR:-}"
APP_BUNDLE="${APP_BUNDLE:-$ROOT/target/release/bundle/macos/bagZ.app}"
DMG_PATH="${DMG_PATH:-}"

usage() {
  cat <<'EOF'
Usage: cef-size-report.sh [options]

Options:
  --cef-dir <path>     Explicit CEF version directory
  --app <path>         Explicit .app path (default: target/release/bundle/macos/bagZ.app)
  --dmg <path>         Explicit .dmg path (default: newest under target/release/bundle/dmg)
  --top <n>            Number of top entries/files to print (default: 20)
  -h, --help           Show this help
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

path_mtime() {
  local path="$1"
  if stat -f '%m' "$path" >/dev/null 2>&1; then
    stat -f '%m' "$path"
  else
    stat -c '%Y' "$path"
  fi
}

is_cef_version_dir() {
  local dir="$1"
  [[ -d "$dir/Chromium Embedded Framework.framework" ]] \
    || [[ -f "$dir/libcef.so" ]] \
    || [[ -f "$dir/libcef.dll" ]]
}

resolve_default_cef_dir() {
  local os arch cache_base platform_dir
  os="$(uname -s)"
  arch="$(uname -m)"

  if [[ -n "${CEF_PATH:-}" ]]; then
    cache_base="$CEF_PATH"
  else
    case "$os" in
      Darwin)
        cache_base="$HOME/Library/Caches/tauri-cef"
        case "$arch" in
          arm64|aarch64) platform_dir="cef_macos_aarch64" ;;
          x86_64|amd64) platform_dir="cef_macos_x86_64" ;;
          *) die "unsupported macOS architecture: $arch" ;;
        esac
        cache_base="$cache_base/$platform_dir"
        ;;
      Linux)
        cache_base="${XDG_CACHE_HOME:-$HOME/.cache}/tauri-cef"
        case "$arch" in
          x86_64|amd64) platform_dir="cef_linux_x86_64" ;;
          arm64|aarch64) platform_dir="cef_linux_aarch64" ;;
          *) die "unsupported Linux architecture: $arch" ;;
        esac
        cache_base="$cache_base/$platform_dir"
        ;;
      *)
        die "automatic CEF resolution is only supported on macOS/Linux for this script"
        ;;
    esac
  fi

  if is_cef_version_dir "$cache_base"; then
    echo "$cache_base"
    return 0
  fi

  [[ -d "$cache_base" ]] || die "CEF cache base does not exist: $cache_base"

  local best_dir="" best_time=0
  while IFS= read -r -d '' candidate; do
    local name mtime
    is_cef_version_dir "$candidate" || continue
    name="$(basename "$candidate")"
    [[ "$name" == *".bak."* ]] && continue
    mtime="$(path_mtime "$candidate")"
    if (( mtime > best_time )); then
      best_dir="$candidate"
      best_time="$mtime"
    fi
  done < <(find "$cache_base" -mindepth 1 -maxdepth 1 -type d -print0 2>/dev/null)

  [[ -n "$best_dir" ]] || die "failed to resolve a CEF version directory under: $cache_base"
  echo "$best_dir"
}

if [[ -z "$DMG_PATH" ]]; then
  DMG_PATH="$(find "$ROOT/target/release/bundle/dmg" -maxdepth 1 -type f -name '*.dmg' 2>/dev/null | head -n 1 || true)"
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --cef-dir)
      CEF_DIR="${2:-}"
      shift 2
      ;;
    --app)
      APP_BUNDLE="${2:-}"
      shift 2
      ;;
    --dmg)
      DMG_PATH="${2:-}"
      shift 2
      ;;
    --top)
      TOP_N="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

if [[ -z "$CEF_DIR" ]]; then
  CEF_DIR="$(resolve_default_cef_dir)"
fi
[[ -d "$CEF_DIR" ]] || die "CEF directory does not exist: $CEF_DIR"

echo "CEF directory: $CEF_DIR"
echo "CEF size: $(du -sh "$CEF_DIR" | awk '{print $1}')"
echo

if [[ -d "$CEF_DIR/Chromium Embedded Framework.framework" ]]; then
  FRAMEWORK_DIR="$CEF_DIR/Chromium Embedded Framework.framework"
  RESOURCES_DIR="$FRAMEWORK_DIR/Resources"
  LIBRARIES_DIR="$FRAMEWORK_DIR/Libraries"

  echo "Top CEF framework directories:"
  du -h -d 3 "$FRAMEWORK_DIR" | sort -h | tail -n "$TOP_N"
  echo

  echo "Largest CEF framework files:"
  find "$FRAMEWORK_DIR" -type f -exec ls -lh {} + | sort -k5 -h | tail -n "$TOP_N"
  echo

  if [[ -d "$RESOURCES_DIR" ]]; then
    if [[ -d "$RESOURCES_DIR/locales" ]]; then
      echo "Locale files (locales/*.pak): $(find "$RESOURCES_DIR/locales" -type f -name '*.pak' | wc -l | tr -d ' ')"
    else
      echo "Locale dirs (*.lproj): $(find "$RESOURCES_DIR" -mindepth 1 -maxdepth 1 -type d -name '*.lproj' | wc -l | tr -d ' ')"
    fi
  fi

  if [[ -d "$LIBRARIES_DIR" ]]; then
    echo "Libraries size: $(du -sh "$LIBRARIES_DIR" | awk '{print $1}')"
  fi
fi

echo
if [[ -d "$APP_BUNDLE" ]]; then
  echo ".app: $APP_BUNDLE"
  echo ".app size: $(du -sh "$APP_BUNDLE" | awk '{print $1}')"
else
  echo ".app not found at: $APP_BUNDLE"
fi

if [[ -n "$DMG_PATH" && -f "$DMG_PATH" ]]; then
  echo ".dmg: $DMG_PATH"
  echo ".dmg size: $(du -sh "$DMG_PATH" | awk '{print $1}')"
else
  echo ".dmg not found"
fi
