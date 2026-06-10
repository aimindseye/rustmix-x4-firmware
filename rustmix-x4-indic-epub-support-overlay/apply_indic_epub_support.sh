#!/usr/bin/env bash
set -euo pipefail

OVERLAY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${1:-.}"
ROOT="$(cd "$ROOT" && pwd)"

if [[ ! -f "$ROOT/Cargo.toml" || ! -d "$ROOT/target-xteink-x4" ]]; then
  echo "error: expected rustmix-x4-firmware repository root: $ROOT" >&2
  exit 1
fi

while IFS= read -r path; do
  [[ -n "$path" ]] || continue
  mkdir -p "$ROOT/$(dirname "$path")"
  cp -p "$OVERLAY_DIR/files/$path" "$ROOT/$path"
done < "$OVERLAY_DIR/CHANGED-FILES.txt"

chmod +x \
  "$ROOT/tools/font_prepare/prepare_book.py" \
  "$ROOT/tools/prepared_epub_smoke/prepare_epub.py" \
  "$ROOT/scripts/validate_prepared_indic_epub_support.sh"

echo "rustmix-x4-indic-epub-support=applied"
