# Rustmix X4 Indic EPUB host-preparation repair

Apply this overlay after the Indic EPUB support overlay.

```bash
unzip -o rustmix-x4-indic-epub-host-preparation-repair.zip
chmod +x rustmix-x4-indic-epub-host-preparation-repair/apply_indic_epub_host_preparation_repair.sh
./rustmix-x4-indic-epub-host-preparation-repair/apply_indic_epub_host_preparation_repair.sh \
  /Users/piyushdaiya/Documents/projects/rustmix-x4-firmware
```

The repair:

- streams Cargo/Rust output instead of hiding it behind a Python `CalledProcessError`;
- prints a shell-quoted replay command;
- preserves extracted EPUB text after a failed preparation attempt;
- loads Gujarati fonts only for books containing Gujarati text;
- requires static Noto TTF faces instead of silently selecting variable-font files.

Static font filenames expected under `--fonts-dir`:

```text
NotoSans-Regular.ttf
NotoSansDevanagari-Regular.ttf      # NotoSansDevanagari-Medium.ttf is also accepted
NotoSansGujarati-Regular.ttf        # required only for Gujarati books
```
