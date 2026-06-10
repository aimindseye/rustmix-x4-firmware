# Rustmix X4 Indic EPUB Generator Output Parser Repair

This cumulative host-side repair fixes a post-generation Python wrapper failure:

```text
ERROR: 'NoneType' object has no attribute 'splitlines'
```

The Rust cache generator may already have completed successfully before that error. The wrapper now tees Cargo output: it remains visible in the terminal while also being retained for metadata parsing.

## Apply

```bash
unzip -o rustmix-x4-indic-epub-generator-output-repair.zip
chmod +x rustmix-x4-indic-epub-generator-output-repair/*.sh

./rustmix-x4-indic-epub-generator-output-repair/apply_indic_epub_generator_output_repair.sh \
  /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware
```

The overlay is idempotent. It changes host-side preparation tooling only. No firmware flash is required.

## Validate an already generated Garud Purana cache

```bash
cd /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware

python3 tools/font_prepare/prepare_book.py validate \
  --cache "/Volumes/NO NAME/FCACHE/FBB52E15" \
  --device-path GARUDPUR.EPU
```

## Optional rerun

After applying the overlay, the original EPUB preparation command can also be rerun safely. The wrapper will print the cache upload summary and automatically validate the generated page-local cache.
