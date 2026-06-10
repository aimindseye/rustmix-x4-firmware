# Rustmix X4 Indic EPUB support overlay

This overlay updates `rustmix-x4-firmware` to render host-prepared EPUB caches containing Devanagari, Sanskrit/Vedic marks, and Gujarati text.

## Scope

- Preserves the existing TXT/EPUB Reader, bookmarks, paging, Wi-Fi Transfer, partition table, and UI-font separation.
- Adds production `cache_format=2` prepared-book caches with FAT 8.3-safe page-local names.
- Loads only the active page's required VFN assets on the X4.
- Keeps compatibility with older global-font `FONTS.IDX` + `PAGES.IDX` smoke caches.
- Routes prepared EPUB bookmark restoration and page jumps through the prepared renderer.
- Extends the host Rustybuzz/fontdue generator for Devanagari extensions, Vedic marks, and Gujarati.
- Adds script auditing and binary cache validation.
- Generates replacement caches through `<BOOKID>.TMP` with `<BOOKID>.BAK` rollback during activation.

No font files or EPUB files are bundled. Use local Noto font files on your computer.

## Apply

```bash
unzip -o rustmix-x4-indic-epub-support-overlay.zip
chmod +x rustmix-x4-indic-epub-support-overlay/apply_indic_epub_support.sh
./rustmix-x4-indic-epub-support-overlay/apply_indic_epub_support.sh /path/to/rustmix-x4-firmware
```

The apply script is idempotent: it copies the same semantic source updates each time.

## Static validation

```bash
cd /path/to/rustmix-x4-firmware
./scripts/validate_prepared_indic_epub_support.sh \
  "/path/to/Garud puran.epub" \
  "/path/to/Srimad Bhagavat Mahapuran Volume 1 Sanskrit Hindi.epub" \
  "/path/to/વાલ્મીકી રામાયણ.epub"
```

## Rust and firmware validation

Run on a machine with your Rust/ESP toolchain:

```bash
cargo fmt --all
cargo fmt --manifest-path tools/prepared_txt_real_vfnt/Cargo.toml
cargo test --manifest-path tools/prepared_txt_real_vfnt/Cargo.toml
cargo check -p target-xteink-x4 --target riscv32imc-unknown-none-elf
cargo build -p target-xteink-x4 --release --target riscv32imc-unknown-none-elf
```

## Prepare SD caches

Use the exact filename/path shown by the X4 browser as `--device-path`. Short `.EPU` names are recommended for the device, for example `GARUDPUR.EPU`, `BHAGV1.EPU`, and `RAMAYAN.EPU`.

```bash
python3 tools/font_prepare/prepare_book.py epub \
  --book "/path/to/Garud puran.epub" \
  --device-path GARUDPUR.EPU \
  --fonts-dir /path/to/noto-font-folder \
  --out /Volumes/<SDCARD>/FCACHE \
  --clean

python3 tools/font_prepare/prepare_book.py epub \
  --book "/path/to/Srimad Bhagavat Mahapuran Volume 1 Sanskrit Hindi.epub" \
  --device-path BHAGV1.EPU \
  --fonts-dir /path/to/noto-font-folder \
  --out /Volumes/<SDCARD>/FCACHE

python3 tools/font_prepare/prepare_book.py epub \
  --book "/path/to/વાલ્મીકી રામાયણ.epub" \
  --device-path RAMAYAN.EPU \
  --fonts-dir /path/to/noto-font-folder \
  --out /Volumes/<SDCARD>/FCACHE
```

The Gujarati sample requires `NotoSansGujarati-Regular.ttf` under `--fonts-dir`, or an explicit `--gujarati-font` path. The first two samples use Devanagari and do not generate Gujarati page assets.

Keep each renamed original book on the SD card at the matching `--device-path`, alongside the generated `/FCACHE/<BOOKID>/` directory.
