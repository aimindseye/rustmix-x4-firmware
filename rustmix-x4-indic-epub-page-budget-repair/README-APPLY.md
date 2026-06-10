# Rustmix X4 Indic EPUB page-budget repair

Apply this overlay after the Indic EPUB support overlay. It is cumulative with the earlier host-preparation diagnostics repair, so applying it after that repair is safe.

```bash
unzip -o rustmix-x4-indic-epub-page-budget-repair.zip
chmod +x rustmix-x4-indic-epub-page-budget-repair/*.sh

./rustmix-x4-indic-epub-page-budget-repair/apply_indic_epub_page_budget_repair.sh \
  /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware
```

The repair changes the host cache generator only. A device firmware rebuild or flash is not required for this particular repair.

## Why the prior run failed

The first production EPUB run shaped page 3 into 1,083 VRUN glyph records. The X4 device page loader intentionally permits at most 1,024 records per prepared page.

The generator now starts a new prepared page before adding a shaped cluster that would exceed any firmware-safe page-local boundary:

- 1,024 VRUN glyph records;
- 24 KiB VRUN bytes;
- 24 KiB page-local Latin, Devanagari, or Gujarati VFNT bytes.

A Devanagari or Gujarati shaping cluster is kept intact when a prepared page is split.

## Validate on a Mac with Cargo installed

```bash
cd /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware
cargo fmt --manifest-path tools/prepared_txt_real_vfnt/Cargo.toml
cargo test --manifest-path tools/prepared_txt_real_vfnt/Cargo.toml
```

## Rerun the first EPUB

```bash
python3 tools/font_prepare/prepare_book.py epub \
  --book "/Users/piyushdaiya/Documents/books/Garud puran.epub" \
  --device-path GARUDPUR.EPU \
  --latin-font "/Users/piyushdaiya/Library/Fonts/NotoSans-Regular.ttf" \
  --devanagari-font "/Users/piyushdaiya/Library/Fonts/NotoSansDevanagari-Medium.ttf" \
  --out "/Volumes/NO NAME/FCACHE" \
  --clean
```

A successful run prints `resource_split_pages=<count>` and completes cache validation.
