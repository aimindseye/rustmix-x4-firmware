# Rustmix X4 Firmware v1.1.0

This release adds host-prepared Indic EPUB rendering for Hindi, Sanskrit, and Gujarati books on the Xteink X4.

## Highlights

- Adds Devanagari EPUB rendering for Hindi and Sanskrit content.
- Adds Gujarati EPUB rendering, including mixed-script books.
- Performs complex Indic shaping on a computer so the constrained X4 firmware does not need to run a large shaping engine on-device.
- Uses page-local FAT 8.3-safe cache assets under `/FCACHE/<BOOKID>`.
- Splits dense shaped pages before firmware-safe glyph, VRUN-byte, or page-font limits are exceeded.
- Cleans macOS AppleDouble `._*` files and `.DS_Store` metadata before cache validation.
- Preserves existing Reader features, TXT/EPUB browsing, bookmarks, settings, Wi-Fi Transfer, and the accepted X4/CrossPoint-compatible partition layout.

## Validated books

- Garud Purana EPUB with Hindi and Sanskrit Devanagari text.
- Srimad Bhagavat Mahapuran Volume 1 EPUB with Hindi Devanagari text.
- Gujarati Valmiki Ramayana EPUB.

## Preparing an Indic EPUB cache

Use static Noto TTF files and place the generated cache at the SD-card root:

```bash
python3 tools/font_prepare/prepare_book.py epub \
  --book "/path/to/book.epub" \
  --device-path GARUDPUR.EPU \
  --latin-font "/path/to/NotoSans-Regular.ttf" \
  --devanagari-font "/path/to/NotoSansDevanagari-Medium.ttf" \
  --out "/Volumes/<SDCARD>/FCACHE" \
  --clean
```

Add `--gujarati-font "/path/to/NotoSansGujarati-Regular.ttf"` for Gujarati books.

See `README.md`, `USERGUIDE.md`, and `SCREENSHOTS.md` for the complete workflow and physically validated device screenshots.
