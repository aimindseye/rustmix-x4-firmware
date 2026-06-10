#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ ! -f "$ROOT/tools/font_prepare/prepare_book.py" ]]; then
  echo "ERROR: expected rustmix-x4-firmware repository root, missing: $ROOT/tools/font_prepare/prepare_book.py" >&2
  exit 1
fi

install -m 0644 "$SCRIPT_DIR/files/tools/font_prepare/prepare_book.py" "$ROOT/tools/font_prepare/prepare_book.py"
install -m 0644 "$SCRIPT_DIR/files/tools/font_prepare/README.md" "$ROOT/tools/font_prepare/README.md"
install -m 0644 "$SCRIPT_DIR/files/USERGUIDE.md" "$ROOT/USERGUIDE.md"

python3 -m py_compile "$ROOT/tools/font_prepare/prepare_book.py"
rm -rf "$ROOT/tools/font_prepare/__pycache__"

echo "indic-epub-host-preparation-repair=applied"
echo "repo=$ROOT"
