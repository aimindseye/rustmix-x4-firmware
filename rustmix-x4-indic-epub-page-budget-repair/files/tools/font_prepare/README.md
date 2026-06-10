# Indic EPUB and TXT Font Preparation

Rustmix X4 keeps complex-script shaping on the host. The firmware renders compact bitmap glyph assets and positioned glyph runs from `/FCACHE/<BOOKID>/`; it does not embed a full Indic shaping engine on the ESP32-C3.

Use this tool for UTF-8 TXT and EPUB books containing Latin, Devanagari, Vedic marks, or Gujarati text.

## Production cache layout

New caches use `cache_format=2` and deterministic FAT 8.3-safe page-local names:

```text
/FCACHE/<BOOKID>/
  META.TXT
  P00000.VRN
  L00000.VFN    # only when page 0 uses Latin glyphs
  D00000.VFN    # only when page 0 uses Devanagari glyphs
  G00000.VFN    # only when page 0 uses Gujarati glyphs
  P00001.VRN
  ...
```

Each page loads only its required font assets. This avoids retaining whole-book Devanagari or Gujarati glyph sets in the X4 heap. During host layout, the generator starts a new page before a shaped cluster would exceed the firmware-safe VRUN glyph-record, VRUN byte, or page-local VFN byte budget. It does not split a Devanagari or Gujarati shaping cluster merely to satisfy a cache limit. The firmware still accepts older `FONTS.IDX` + `PAGES.IDX` smoke caches for backward compatibility.

## Font prerequisites

Use locally installed or downloaded **static** Google Noto TTF faces:

```text
NotoSans-Regular.ttf
NotoSansDevanagari-Regular.ttf
NotoSansGujarati-Regular.ttf      # required only for Gujarati books
```

Do not use files named like `NotoSans-VariableFont_wdth,wght.ttf`. The cache raster stage intentionally consumes one static TTF face per script so host preparation is deterministic. Font files are inputs on your computer. They are not committed into this repository.

## Audit a book

Audit script coverage without building a cache:

```bash
python3 tools/font_prepare/prepare_book.py audit \
  --book /path/to/BOOK.EPUB
```

## Prepare a Devanagari EPUB

Use the exact 8.3 filename shown by the X4 browser as `--device-path`:

```bash
python3 tools/font_prepare/prepare_book.py epub \
  --book /path/to/BOOK.EPUB \
  --device-path GARUDPUR.EPU \
  --fonts-dir /path/to/noto-font-folder \
  --out /tmp/FCACHE \
  --clean
```

## Prepare a Gujarati EPUB

The same command detects Gujarati characters and requires `NotoSansGujarati-Regular.ttf` under `--fonts-dir`. An explicit path also works:

```bash
python3 tools/font_prepare/prepare_book.py epub \
  --book /path/to/GUJARATI.EPUB \
  --device-path RAMAYAN.EPU \
  --latin-font /path/to/NotoSans-Regular.ttf \
  --devanagari-font /path/to/NotoSansDevanagari-Regular.ttf \
  --gujarati-font /path/to/NotoSansGujarati-Regular.ttf \
  --out /tmp/FCACHE
```

## Prepare UTF-8 TXT

```bash
python3 tools/font_prepare/prepare_book.py txt \
  --book /path/to/BOOK.TXT \
  --device-path BOOK.TXT \
  --fonts-dir /path/to/noto-font-folder \
  --out /tmp/FCACHE
```

## Validate a generated cache

```bash
python3 tools/font_prepare/prepare_book.py validate \
  --cache /tmp/FCACHE/<BOOKID> \
  --device-path GARUDPUR.EPU
```

The validator checks metadata, FAT 8.3 filenames, page limits, page-local font limits, VRUN font slots, and every referenced glyph.

## Copy to SD

Copy the generated directory to:

```text
/FCACHE/<BOOKID>/
```

Keep the original `.EPU`, `.EPUB`, or `.TXT` file at the exact `--device-path` used during preparation. The preparation command prints the computed `<BOOKID>` and an upload summary.

## Repository validation

```bash
scripts/validate_prepared_indic_epub_support.sh \
  /path/to/DEVANAGARI.EPUB \
  /path/to/GUJARATI.EPUB
```

When Cargo is installed, the script also runs host-shaper unit tests. Use the standard root firmware build commands for the target build.
