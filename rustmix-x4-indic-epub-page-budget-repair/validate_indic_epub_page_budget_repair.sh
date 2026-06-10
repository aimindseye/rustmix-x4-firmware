#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
cd "$ROOT"

python3 -m py_compile tools/font_prepare/prepare_book.py tools/prepared_epub_smoke/prepare_epub.py
rm -rf tools/font_prepare/__pycache__ tools/prepared_epub_smoke/__pycache__

grep -q 'struct PageBudget' tools/prepared_txt_real_vfnt/src/main.rs
grep -q 'fn projected(&self, resources: &\[GlyphResource\])' tools/prepared_txt_real_vfnt/src/main.rs
grep -q 'single shaped cluster at byte' tools/prepared_txt_real_vfnt/src/main.rs
grep -q 'resource_split_pages' tools/prepared_txt_real_vfnt/src/main.rs
grep -q 'page_budget_splits_before_firmware_glyph_limit' tools/prepared_txt_real_vfnt/src/main.rs
grep -q 'page_budget_counts_repeated_bitmap_once' tools/prepared_txt_real_vfnt/src/main.rs
grep -q 'page_budget_rejects_page_local_font_overflow' tools/prepared_txt_real_vfnt/src/main.rs

./scripts/validate_prepared_indic_epub_support.sh
./scripts/check_repo_hygiene.sh
./scripts/audit_remaining_pulp_runtime_dependencies.sh
./scripts/validate_x4_standard_partition_table_compatibility.sh
./scripts/validate_x4_flash_ota_slot_policy.sh

echo 'indic-epub-page-budget-repair=passed'
