# Rustmix X4 Indic EPUB macOS Metadata Repair

This cumulative host-tool overlay fixes validation of prepared `/FCACHE/<BOOKID>/`
directories written to FAT-formatted SD cards by macOS.

macOS may create AppleDouble `._*` sidecars and `.DS_Store` in a generated cache.
Those files are not Reader assets. The preparation and validation workflow now
removes them before listing or inspecting cache files and ignores a sidecar if it
reappears during validation.

No firmware runtime files change. A device rebuild or flash is not required.

## Apply

```bash
unzip -o rustmix-x4-indic-epub-macos-metadata-repair.zip
chmod +x rustmix-x4-indic-epub-macos-metadata-repair/*.sh

./rustmix-x4-indic-epub-macos-metadata-repair/apply_indic_epub_macos_metadata_repair.sh \
  /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware
```

## Validate an existing generated cache

```bash
cd /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware
python3 tools/font_prepare/prepare_book.py validate \
  --cache "/Volumes/NO NAME/FCACHE/FBB52E15" \
  --device-path GARUDPUR.EPU
```
