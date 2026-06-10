# Prepared EPUB Smoke Compatibility Path

The original smoke command remains available, but it now delegates to the production Indic EPUB workflow in `tools/font_prepare/prepare_book.py`.

Generate a tiny smoke EPUB:

```bash
python3 tools/prepared_epub_smoke/create_smoke_epub.py /tmp/MIXED_EPUB.EPUB
```

Prepare it with local Noto font files:

```bash
python3 tools/prepared_epub_smoke/prepare_epub.py \
  --epub /tmp/MIXED_EPUB.EPUB \
  --device-path MIXED_EP.EPU \
  --latin-font /path/to/NotoSans-Regular.ttf \
  --devanagari-font /path/to/NotoSansDevanagari-Regular.ttf \
  --out /tmp/FCACHE
```

New real-book caches use `cache_format=2` page-local assets such as `P00000.VRN`, `L00000.VFN`, `D00000.VFN`, and optional `G00000.VFN`. Legacy smoke caches with `FONTS.IDX`, `PAGES.IDX`, `LAT18.VFN`, and `DEV22.VFN` remain readable by the firmware.
