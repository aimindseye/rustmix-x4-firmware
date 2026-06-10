# Rustmix X4 Firmware

Rustmix is a neutral Rust reference firmware for the Xteink X4 e-ink reader. It is intended for people who want to use the X4 as an open reader and for developers who want a practical Rust starting point for ESP32-C3 e-ink firmware.

Rustmix includes a reader, Wi-Fi transfer, first-boot SD provisioning, custom fonts, dictionary data, flashcards, random sleep images, and an SD-loaded Lua app model.

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

See [`SCREENSHOTS.md`](SCREENSHOTS.md) for reference screenshots of the home dashboard, reader, dictionary, calendar, network setup, system settings, and sleep-image screens. Screenshot images are stored in `screenshots/`.

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
```

First boot seeds missing default files only. User files are preserved.

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

Create a compact release folder and zip with firmware, flashing notes, partition files, and the SD-card sleep-image pack:

```bash
scripts/create_rustmix_release_package.sh
```

## Developer references and inspirations

Rustmix is inspired by or informed by:

- pulp-os: minimal Xteink X4 Rust firmware lineage.
- Biscuit: simple e-reader product flow and dashboard ideas.
- CrossInk: compact e-ink UI patterns for headers, rows, tabs, and footer-safe screens.
- CrossPoint Reader: Xteink-compatible firmware and partition-layout awareness.
- Lua-reader experiments such as CrossLuaReader-style SD apps, bounded host APIs, and data-driven app folders.

Rustmix is not official Xteink firmware. Flashing custom firmware can fail on unsupported devices. Keep recovery options available.
