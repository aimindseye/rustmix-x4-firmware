# Rustmix X4 Firmware

Rustmix is a neutral Rust reference firmware for the Xteink X4 e-ink reader. It is intended for people who want to use the X4 as an open reader and for developers who want a practical Rust starting point for ESP32-C3 e-ink firmware.

Rustmix includes a reader, Wi-Fi transfer, first-boot SD provisioning, custom fonts, dictionary data, flashcards, random sleep images, and an SD-loaded Lua app model. Host-prepared EPUB caches add properly shaped Hindi, Sanskrit, and Gujarati book rendering without running a large Indic shaping engine on the device.

## Included capabilities

- ESP32-C3 Rust firmware target for Xteink X4.
- SSD1677 display path for the 800 × 480 e-paper panel.
- X4/CrossPoint-compatible partition table.
- Reader with recent books, bookmarks, TXT/EPUB support, cache progress, reader settings, and host-prepared Devanagari/Gujarati EPUB caches.
- Wi-Fi Transfer for SD-card access without removing the card.
- First-boot SD setup under `/RUSTMIX`.
- SD-loaded Lua apps under `/RUSTMIX/APPS`.
- Custom VFN fonts under `/RUSTMIX/FONTS`.
- Dictionary prefix shards under `/RUSTMIX/APPS/DICT`.
- Flashcards with text and image topics under `/RUSTMIX/APPS/FLASHCRD`.
- Random sleep images under `/RUSTMIX/SLEEP`.

## Repository documentation policy

Release documentation is consolidated into these root files:

```text
README.md
ARCHITECTURE.md
USERGUIDE.md
SCOPE.md
ROADMAP.md
SCREENSHOTS.md
```

The repository intentionally keeps first-release documentation in root files only, so CI hygiene can keep release guidance in one visible place. Screenshot references live in [`SCREENSHOTS.md`](SCREENSHOTS.md), with image assets in `screenshots/`.

## Screenshots

See [`SCREENSHOTS.md`](SCREENSHOTS.md) for reference screenshots of the home dashboard, reader, Hindi and Gujarati EPUB rendering, dictionary, calendar, network setup, system settings, and sleep-image screens. Screenshot images are stored in `screenshots/`.

## SD-card layout

Rustmix creates and uses this SD-card root:

```text
/RUSTMIX
/RUSTMIX/APPS
/RUSTMIX/FONTS
/RUSTMIX/SLEEP
/RUSTMIX/CACHE
/RUSTMIX/STATE
/RUSTMIX/SETTINGS.TXT
/RUSTMIX/TIME.TXT

/FCACHE                 # host-prepared EPUB caches; separate from /RUSTMIX
/FCACHE/<BOOKID>/META.TXT
/FCACHE/<BOOKID>/P00000.VRN
/FCACHE/<BOOKID>/L00000.VFN
/FCACHE/<BOOKID>/D00000.VFN
/FCACHE/<BOOKID>/G00000.VFN   # optional, for Gujarati pages
```

First boot seeds missing default files only. User files are preserved.

## Hindi and Gujarati EPUB support

Rustmix supports host-prepared EPUB caches for Devanagari and Gujarati text. This covers Hindi, Sanskrit, Devanagari extensions, Vedic marks, Gujarati, and mixed-script pages. Complex shaping is performed on a computer with Rustybuzz; the X4 loads compact page-local assets from the SD card.

Use static Noto TTF files and the exact FAT 8.3 filename shown by the X4 file browser as `--device-path`. Generate caches directly into the top-level `/FCACHE` directory on the mounted SD card:

```bash
python3 tools/font_prepare/prepare_book.py epub \
  --book "/path/to/book.epub" \
  --device-path GARUDPUR.EPU \
  --latin-font "/path/to/NotoSans-Regular.ttf" \
  --devanagari-font "/path/to/NotoSansDevanagari-Medium.ttf" \
  --out "/Volumes/<SDCARD>/FCACHE" \
  --clean
```

For a Gujarati EPUB, add:

```bash
  --gujarati-font "/path/to/NotoSansGujarati-Regular.ttf"
```

Keep the renamed `.EPU` source book on the SD card and preserve the generated `/FCACHE/<BOOKID>` directory. The cache generator emits FAT 8.3-safe page-local files and cleans macOS AppleDouble sidecars before validation. Full preparation instructions are in [`USERGUIDE.md`](USERGUIDE.md#read-devanagari-and-gujarati-epub-books).

Validated device screenshots:

- [Gujarati Ramayana EPUB](screenshots/epub-gujarati-ramayana.jpg)
- [Hindi Bhagavat Mahapuran EPUB](screenshots/epub-hindi-bhagavat.jpg)
- [Hindi/Sanskrit Garud Purana EPUB](screenshots/epub-hindi-garud-purana.jpg)

## Build latest firmware

Install Rust and the X4 target, then build:

```bash
rustup target add riscv32imc-unknown-none-elf
cargo install espflash --locked
cargo fmt --all
cargo build -p target-xteink-x4 --release --target riscv32imc-unknown-none-elf
```

Create the release firmware file:

```bash
scripts/create_rustfirmware_bin.sh
```

The output is:

```text
dist/rustmix-x4/rustfirmware.bin
```

## Flashing

Find the USB port:

```bash
ls /dev/ttyACM* 2>/dev/null || true
```

First install / full deploy:

```bash
scripts/flash_x4_release_bin.sh dist/rustmix-x4/rustfirmware.bin /dev/ttyACM0
```

Normal later app-only updates:

```bash
scripts/flash_x4_rustmix_app0.sh /dev/ttyACM0
```

## Release bundle

Create the release firmware files with the repository script, then zip the generated folder for an optional GitHub release asset:

```bash
scripts/create_rustfirmware_bin.sh
(cd dist && rm -f rustmix-x4-rustfirmware.zip && zip -r rustmix-x4-rustfirmware.zip rustmix-x4)
```

The version-tag GitHub Actions workflow also builds `rustfirmware.bin`, the ELF file, checksums, flashing notes, `espflash.toml`, and the standard X4 partition files as a downloadable workflow artifact.

## Developer references and inspirations

Rustmix is inspired by or informed by:

- pulp-os: minimal Xteink X4 Rust firmware lineage.
- Biscuit: simple e-reader product flow and dashboard ideas.
- CrossInk: compact e-ink UI patterns for headers, rows, tabs, and footer-safe screens.
- CrossPoint Reader: Xteink-compatible firmware and partition-layout awareness.
- Lua-reader experiments such as CrossLuaReader-style SD apps, bounded host APIs, and data-driven app folders.

Rustmix is not official Xteink firmware. Flashing custom firmware can fail on unsupported devices. Keep recovery options available.
