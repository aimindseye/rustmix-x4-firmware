#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

required=(
  "$ROOT/tools/font_prepare/prepare_book.py"
  "$ROOT/tools/font_prepare/README.md"
  "$ROOT/tools/prepared_txt_real_vfnt/src/main.rs"
  "$ROOT/scripts/validate_prepared_indic_epub_support.sh"
)
for path in "${required[@]}"; do
  if [[ ! -f "$path" ]]; then
    echo "ERROR: expected rustmix-x4-firmware repository with Indic EPUB support, missing: $path" >&2
    exit 1
  fi
done

install -m 0644 "$SCRIPT_DIR/files/tools/font_prepare/prepare_book.py" \
  "$ROOT/tools/font_prepare/prepare_book.py"
install -m 0644 "$SCRIPT_DIR/files/tools/font_prepare/README.md" \
  "$ROOT/tools/font_prepare/README.md"
install -m 0755 "$SCRIPT_DIR/files/scripts/validate_indic_epub_macos_metadata_repair.sh" \
  "$ROOT/scripts/validate_indic_epub_macos_metadata_repair.sh"
install -m 0755 "$SCRIPT_DIR/files/scripts/validate_prepared_indic_epub_support.sh" \
  "$ROOT/scripts/validate_prepared_indic_epub_support.sh"

python3 -m py_compile "$ROOT/tools/font_prepare/prepare_book.py"
rm -rf "$ROOT/tools/font_prepare/__pycache__"

"$ROOT/scripts/validate_indic_epub_macos_metadata_repair.sh"

echo "indic-epub-macos-metadata-repair=applied"
echo "repo=$ROOT"
