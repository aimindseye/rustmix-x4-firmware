#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cleanup() {
  rm -rf tools/font_prepare/__pycache__ tools/prepared_epub_smoke/__pycache__
}
trap cleanup EXIT

python3 -m py_compile \
  tools/font_prepare/prepare_book.py \
  tools/prepared_epub_smoke/prepare_epub.py

for book in "$@"; do
  python3 tools/font_prepare/prepare_book.py audit --book "$book"
done

rg -q 'CACHE_FORMAT_PAGE_LOCAL: usize = 2' \
  target-xteink-x4/src/rustmix_x4/x4_apps/apps/reader/prepared_txt.rs
rg -q 'FONT_GUJARATI' \
  target-xteink-x4/src/rustmix_x4/x4_apps/apps/reader/prepared_txt.rs \
  tools/prepared_txt_real_vfnt/src/main.rs
rg -q 'script::GUJARATI' tools/prepared_txt_real_vfnt/src/main.rs
rg -q 'page_local_file_name' \
  target-xteink-x4/src/rustmix_x4/x4_apps/apps/reader/prepared_txt.rs \
  tools/prepared_txt_real_vfnt/src/main.rs

if command -v cargo >/dev/null 2>&1; then
  cargo test --manifest-path tools/prepared_txt_real_vfnt/Cargo.toml
else
  echo 'SKIP: cargo is not installed; host-shaper Rust tests were not run.'
fi

echo 'prepared-indic-epub-support=passed'
