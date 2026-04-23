#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

PROFILE="${CEF_SLIM_PROFILE:-safe}"
STAGE_ROOT="${CEF_STAGE_ROOT:-$ROOT/target/cef-stage}"
KEEP_LOCALES="${CEF_KEEP_LOCALES:-en.lproj,en_GB.lproj}"
SOURCE_BASE="${CEF_SOURCE_BASE:-}"
SOURCE_DIR="${CEF_SOURCE_DIR:-}"
PRINT_CEF_BASE=0
PRINT_STAGE_DIR=0
QUIET=0

usage() {
  cat <<'EOF'
Usage: cef-stage-slim.sh [options]

Options:
  --profile <safe|aggressive>  Slim profile (default: safe)
  --stage-root <path>          Staging root directory (default: target/cef-stage)
  --keep-locales <csv>         Locales to keep, e.g. "en.lproj,en_GB.lproj"
  --source-base <path>         CEF base directory (version subdirs or a version dir)
  --source-dir <path>          Explicit CEF version directory
  --print-cef-base             Print staged CEF base path (for CEF_PATH)
  --print-stage-dir            Print staged CEF version directory
  --quiet                      Suppress info logs
  -h, --help                   Show this help

Env:
  CEF_PATH            If set and --source-base/--source-dir are unset, used as source base.
  CEF_SLIM_PROFILE    Default profile if --profile is not provided.
  CEF_STAGE_ROOT      Default stage root if --stage-root is not provided.
  CEF_KEEP_LOCALES    Default keep-locales CSV if --keep-locales is not provided.
EOF
}

log() {
  if [[ "$QUIET" -eq 0 ]]; then
    echo "info: $*" >&2
  fi
}

die() {
  echo "error: $*" >&2
  exit 1
}

trim_ws() {
  local value="$1"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  printf '%s' "$value"
}

path_mtime() {
  local path="$1"
  if stat -f '%m' "$path" >/dev/null 2>&1; then
    stat -f '%m' "$path"
  else
    stat -c '%Y' "$path"
  fi
}

dir_size_kib() {
  local dir="$1"
  du -sk "$dir" | awk '{ print $1 }'
}

human_from_kib() {
  local kib="$1"
  awk -v kib="$kib" '
    BEGIN {
      split("KiB MiB GiB TiB PiB", u, " ");
      value = kib + 0.0;
      unit = 1;
      while (value >= 1024.0 && unit < 5) {
        value /= 1024.0;
        unit++;
      }
      printf "%.1f %s", value, u[unit];
    }'
}

is_cef_version_dir() {
  local dir="$1"
  [[ -d "$dir/Chromium Embedded Framework.framework" ]] \
    || [[ -f "$dir/libcef.so" ]] \
    || [[ -f "$dir/libcef.dll" ]]
}

detect_platform_cache_subdir() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "cef_macos_aarch64" ;;
        x86_64|amd64) echo "cef_macos_x86_64" ;;
        *) die "unsupported macOS architecture: $arch" ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64|amd64) echo "cef_linux_x86_64" ;;
        arm64|aarch64) echo "cef_linux_aarch64" ;;
        *) die "unsupported Linux architecture: $arch" ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*)
      case "$arch" in
        x86_64|amd64) echo "cef_windows_x86_64" ;;
        arm64|aarch64) echo "cef_windows_aarch64" ;;
        *) die "unsupported Windows architecture: $arch" ;;
      esac
      ;;
    *)
      die "unsupported OS: $os"
      ;;
  esac
}

default_tauri_cef_base() {
  local cache_root subdir
  subdir="$(detect_platform_cache_subdir)"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    cache_root="$HOME/Library/Caches"
  else
    cache_root="${XDG_CACHE_HOME:-$HOME/.cache}"
  fi
  echo "$cache_root/tauri-cef/$subdir"
}

find_latest_cef_version_dir() {
  local base="$1"
  local best_dir="" best_time=0
  local fallback_dir="" fallback_time=0
  while IFS= read -r -d '' candidate; do
    local name mtime
    is_cef_version_dir "$candidate" || continue
    name="$(basename "$candidate")"
    mtime="$(path_mtime "$candidate")"
    if [[ "$name" == *".bak."* ]]; then
      if (( mtime > fallback_time )); then
        fallback_dir="$candidate"
        fallback_time="$mtime"
      fi
      continue
    fi
    if (( mtime > best_time )); then
      best_dir="$candidate"
      best_time="$mtime"
    fi
  done < <(find "$base" -mindepth 1 -maxdepth 1 -type d -print0 2>/dev/null)

  if [[ -n "$best_dir" ]]; then
    echo "$best_dir"
    return 0
  fi
  if [[ -n "$fallback_dir" ]]; then
    echo "$fallback_dir"
    return 0
  fi
  return 1
}

resolve_source_dir() {
  local resolved_source_base="$SOURCE_BASE"

  if [[ -n "$SOURCE_DIR" ]]; then
    [[ -d "$SOURCE_DIR" ]] || die "--source-dir does not exist: $SOURCE_DIR"
    is_cef_version_dir "$SOURCE_DIR" || die "--source-dir is not a CEF version directory: $SOURCE_DIR"
    echo "$SOURCE_DIR"
    return 0
  fi

  if [[ -z "$resolved_source_base" && -n "${CEF_PATH:-}" ]]; then
    resolved_source_base="$CEF_PATH"
  fi
  if [[ -z "$resolved_source_base" ]]; then
    resolved_source_base="$(default_tauri_cef_base)"
  fi

  [[ -d "$resolved_source_base" ]] || die "CEF source base directory does not exist: $resolved_source_base"

  if is_cef_version_dir "$resolved_source_base"; then
    echo "$resolved_source_base"
    return 0
  fi

  local version_dir
  version_dir="$(find_latest_cef_version_dir "$resolved_source_base" || true)"
  [[ -n "$version_dir" ]] || die "failed to locate a CEF version directory under: $resolved_source_base"
  echo "$version_dir"
}

copy_tree() {
  local src="$1"
  local dst="$2"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete "$src/" "$dst/"
  else
    mkdir -p "$dst"
    cp -R "$src/." "$dst/"
  fi
}

REMOVED_MANIFEST=""
SUMMARY_FILE=""
STAGE_DIR=""

remove_path() {
  local path="$1"
  if [[ -e "$path" || -L "$path" ]]; then
    local rel="${path#$STAGE_DIR/}"
    printf '%s\n' "$rel" >>"$REMOVED_MANIFEST"
    rm -rf "$path"
  fi
}

prune_locales() {
  local resources_dir="$1"
  local -a keep_locales=()
  IFS=',' read -r -a keep_locales <<<"$KEEP_LOCALES"
  if [[ "${#keep_locales[@]}" -eq 0 ]]; then
    keep_locales=("en.lproj")
  fi

  local i
  for i in "${!keep_locales[@]}"; do
    keep_locales[$i]="$(trim_ws "${keep_locales[$i]}")"
  done

  while IFS= read -r -d '' locale_dir; do
    local locale_name keep=0 locale
    locale_name="$(basename "$locale_dir")"
    for locale in "${keep_locales[@]}"; do
      if [[ "$locale_name" == "$locale" ]]; then
        keep=1
        break
      fi
    done
    if [[ "$keep" -eq 0 ]]; then
      remove_path "$locale_dir"
    fi
  done < <(find "$resources_dir" -mindepth 1 -maxdepth 1 -type d -name '*.lproj' -print0)

  local locales_dir="$resources_dir/locales"
  if [[ -d "$locales_dir" ]]; then
    local -a keep_paks=()
    local locale
    for locale in "${keep_locales[@]}"; do
      case "$locale" in
        *.pak) keep_paks+=("$locale") ;;
        *.lproj) keep_paks+=("${locale%.lproj}.pak") ;;
        *) keep_paks+=("${locale}.pak") ;;
      esac
    done
    if [[ "${#keep_paks[@]}" -eq 0 ]]; then
      keep_paks=("en-US.pak" "en.pak")
    fi

    while IFS= read -r -d '' locale_file; do
      local locale_name keep=0 keep_pak
      locale_name="$(basename "$locale_file")"
      for keep_pak in "${keep_paks[@]}"; do
        if [[ "$locale_name" == "$keep_pak" ]]; then
          keep=1
          break
        fi
      done
      if [[ "$keep" -eq 0 ]]; then
        remove_path "$locale_file"
      fi
    done < <(find "$locales_dir" -mindepth 1 -maxdepth 1 -type f -name '*.pak' -print0)
  fi
}

prune_safe() {
  local framework_dir="$STAGE_DIR/Chromium Embedded Framework.framework"
  local resources_dir="$framework_dir/Resources"
  [[ -d "$framework_dir" ]] || die "missing CEF framework in stage dir: $framework_dir"
  [[ -d "$resources_dir" ]] || die "missing CEF Resources directory: $resources_dir"

  local required
  for required in "resources.pak" "chrome_100_percent.pak" "chrome_200_percent.pak"; do
    [[ -f "$resources_dir/$required" ]] || die "required CEF resource missing: $required"
  done

  prune_locales "$resources_dir"
}

prune_aggressive() {
  local framework_dir="$STAGE_DIR/Chromium Embedded Framework.framework"
  local libraries_dir="$framework_dir/Libraries"
  local resources_dir="$framework_dir/Resources"

  if [[ -d "$libraries_dir" ]]; then
    local pattern file
    for pattern in "libEGL*" "libGLESv2*" "libvk_swiftshader*" "vk_swiftshader_icd*"; do
      for file in "$libraries_dir"/$pattern; do
        [[ -e "$file" || -L "$file" ]] || continue
        remove_path "$file"
      done
    done
  fi

  if [[ -d "$resources_dir" ]]; then
    local pattern file
    for pattern in "gpu_shader_cache.bin" "vk_swiftshader_icd.json"; do
      for file in "$resources_dir"/$pattern; do
        [[ -e "$file" || -L "$file" ]] || continue
        remove_path "$file"
      done
    done
    for pattern in "swiftshader" "angledata"; do
      for file in "$resources_dir"/$pattern; do
        [[ -d "$file" ]] || continue
        remove_path "$file"
      done
    done
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --stage-root)
      STAGE_ROOT="${2:-}"
      shift 2
      ;;
    --keep-locales)
      KEEP_LOCALES="${2:-}"
      shift 2
      ;;
    --source-base)
      SOURCE_BASE="${2:-}"
      shift 2
      ;;
    --source-dir)
      SOURCE_DIR="${2:-}"
      shift 2
      ;;
    --print-cef-base)
      PRINT_CEF_BASE=1
      shift
      ;;
    --print-stage-dir)
      PRINT_STAGE_DIR=1
      shift
      ;;
    --quiet)
      QUIET=1
      shift
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

case "$PROFILE" in
  safe|aggressive) ;;
  *) die "--profile must be one of: safe, aggressive" ;;
esac

SOURCE_DIR="$(resolve_source_dir)"
VERSION="$(basename "$SOURCE_DIR")"
STAGE_BASE="${STAGE_ROOT%/}/$PROFILE"
STAGE_DIR="$STAGE_BASE/$VERSION"

log "using source CEF directory: $SOURCE_DIR"
log "staging profile '$PROFILE' to: $STAGE_DIR"

mkdir -p "$STAGE_BASE"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"
copy_tree "$SOURCE_DIR" "$STAGE_DIR"

SIZE_BEFORE_KIB="$(dir_size_kib "$STAGE_DIR")"

REMOVED_MANIFEST="$STAGE_DIR/.cef-slim-removed-$PROFILE.txt"
SUMMARY_FILE="$STAGE_DIR/.cef-slim-summary.txt"
{
  echo "# profile=$PROFILE"
  echo "# source_dir=$SOURCE_DIR"
  echo "# stage_dir=$STAGE_DIR"
  echo "# keep_locales=$KEEP_LOCALES"
} >"$REMOVED_MANIFEST"

prune_safe
if [[ "$PROFILE" == "aggressive" ]]; then
  prune_aggressive
fi

SIZE_AFTER_KIB="$(dir_size_kib "$STAGE_DIR")"
REMOVED_COUNT="$(grep -vc '^#' "$REMOVED_MANIFEST" || true)"
SAVED_KIB=$((SIZE_BEFORE_KIB - SIZE_AFTER_KIB))

{
  echo "profile=$PROFILE"
  echo "source_dir=$SOURCE_DIR"
  echo "stage_base=$STAGE_BASE"
  echo "stage_dir=$STAGE_DIR"
  echo "keep_locales=$KEEP_LOCALES"
  echo "size_before_kib=$SIZE_BEFORE_KIB"
  echo "size_after_kib=$SIZE_AFTER_KIB"
  echo "saved_kib=$SAVED_KIB"
  echo "removed_count=$REMOVED_COUNT"
  echo "removed_manifest=$REMOVED_MANIFEST"
} >"$SUMMARY_FILE"

log "removed entries: $REMOVED_COUNT"
log "size before: $(human_from_kib "$SIZE_BEFORE_KIB")"
log "size after:  $(human_from_kib "$SIZE_AFTER_KIB")"
log "saved:       $(human_from_kib "$SAVED_KIB")"
log "removed manifest: $REMOVED_MANIFEST"
log "summary: $SUMMARY_FILE"
log "set CEF_PATH to stage base when building: $STAGE_BASE"

if [[ "$PRINT_CEF_BASE" -eq 1 ]]; then
  printf '%s\n' "$STAGE_BASE"
fi
if [[ "$PRINT_STAGE_DIR" -eq 1 ]]; then
  printf '%s\n' "$STAGE_DIR"
fi
