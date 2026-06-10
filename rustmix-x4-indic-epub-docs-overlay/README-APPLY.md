# Rustmix X4 Indic EPUB documentation and screenshots overlay

This documentation-only overlay adds physically validated Hindi and Gujarati EPUB screenshots, updates `SCREENSHOTS.md`, expands the README Indic EPUB section, documents the top-level `/FCACHE` layout, and replaces an obsolete release-bundle command with commands backed by scripts present in the current repository.

## Apply

```bash
unzip -o rustmix-x4-indic-epub-docs-overlay.zip
chmod +x rustmix-x4-indic-epub-docs-overlay/*.sh
./rustmix-x4-indic-epub-docs-overlay/apply_indic_epub_docs.sh \
  /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware
```

## Validate

```bash
./rustmix-x4-indic-epub-docs-overlay/validate_indic_epub_docs.sh \
  /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware
```

This overlay does not modify firmware runtime code. Rebuild firmware before publishing a release so the release asset reflects the current repository state.
