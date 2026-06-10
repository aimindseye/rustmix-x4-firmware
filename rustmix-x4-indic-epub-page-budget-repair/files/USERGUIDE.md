# Rustmix User Guide

## Screenshots

See [`SCREENSHOTS.md`](SCREENSHOTS.md) for screen-by-screen references.

## Build from source

```bash
sudo apt update
sudo apt install -y unzip build-essential pkg-config libudev-dev python3 python3-pip
rustup target add riscv32imc-unknown-none-elf
cargo install espflash --locked
cargo fmt --all
cargo build -p target-xteink-x4 --release --target riscv32imc-unknown-none-elf
scripts/create_rustfirmware_bin.sh
```

The firmware file is:

```text
dist/rustmix-x4/rustfirmware.bin
```

## Flash over USB

On Ubuntu the X4 usually appears as `/dev/ttyACM0`.

```bash
ls /dev/ttyACM* 2>/dev/null || true
scripts/flash_x4_release_bin.sh dist/rustmix-x4/rustfirmware.bin /dev/ttyACM0
```

For later app-only updates:

```bash
scripts/flash_x4_rustmix_app0.sh /dev/ttyACM0
```

## First boot

Rustmix creates this SD-card layout automatically:

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

Provisioning seeds missing default files only. Existing user files are preserved.

## Reader loading and cache progress

When opening EPUB books, Rustmix shows loading and cache progress such as `Cache 1/33` while the current chapter is prepared. The reader yields between bounded work units so Back can leave the reader during a long open or resume.

## Read Devanagari and Gujarati EPUB books

Indic shaping is prepared on a computer so the X4 can render Hindi, Sanskrit, Vedic-mark, and Gujarati text without running a large shaping engine on-device. Generate a cache with local static Noto TTF faces, using the exact filename shown by the X4 file browser as `--device-path`:

```bash
python3 tools/font_prepare/prepare_book.py epub \
  --book /path/to/BOOK.EPUB \
  --device-path GARUDPUR.EPU \
  --fonts-dir /path/to/noto-font-folder \
  --out /tmp/FCACHE
```

Copy the generated book directory to:

```text
/FCACHE/<BOOKID>/
```

Keep the original book at the exact `--device-path`. New caches use page-local 8.3-safe assets (`P00000.VRN`, `L00000.VFN`, `D00000.VFN`, and optional `G00000.VFN`). Gujarati font assets are generated only for pages that use Gujarati glyphs. Host pagination also starts a new page before a shaped cluster would exceed a firmware-safe page budget, so dense Sanskrit or Gujarati pages do not overflow the device loader.

## Wi-Fi Transfer

Use Wi-Fi Transfer to copy files into `/RUSTMIX` without removing the SD card. Keep folder and file names uppercase and 8.3-safe when possible.

## Add Lua apps

Create a folder under `/RUSTMIX/APPS`:

```text
/RUSTMIX/APPS/MYAPP
/RUSTMIX/APPS/MYAPP/APP.TOM
/RUSTMIX/APPS/MYAPP/MAIN.LUA
```

Example `APP.TOM`:

```toml
id = "myapp"
name = "My App"
category = "Tools"
type = "activity"
version = "0.1.0"
entry = "MAIN.LUA"
capabilities = ["display", "input", "storage"]
```

Example `MAIN.LUA`:

```lua
display_title = "My App"
display_line1 = "Hello from Rustmix Lua."
display_line2 = "Edit MAIN.LUA to customize this app."
display_footer = "Back: Home"
```

Reboot or reopen the app catalog after copying a new app.

## Add custom fonts

Copy VFN files into:

```text
/RUSTMIX/FONTS
```

Update:

```text
/RUSTMIX/FONTS/MANIFEST.TXT
```

Example manifest rows:

```text
INTER14|Inter UI 14|ui|INTER14.VFN
LEXEND18|Lexend 18|book|LEXEND18.VFN
```

Use Settings to select SD font source where supported. Keep VFN file names 8.3-safe.

## Add dictionary data

Build dictionary shards on your computer:

```bash
python3 tools/build_dictionary_sd_pack.py dictionary.json out-dict
python3 tools/check_dictionary_sd_layout.py out-dict
```

Copy the generated files to:

```text
/RUSTMIX/APPS/DICT
```

Required layout:

```text
/RUSTMIX/APPS/DICT/INDEX.TXT
/RUSTMIX/APPS/DICT/DICT.JSN
/RUSTMIX/APPS/DICT/DATA/A.JSN
/RUSTMIX/APPS/DICT/DATA/B.JSN
...
```

If dictionary lookup reports an empty shard, check that the matching `DATA/*.JSN` file is present and non-empty.

## Add flashcard topics

Flashcards are topic-based. The topic list is stored at:

```text
/RUSTMIX/APPS/FLASHCRD/TOPICS/INDEX.TXT
```

Rows use:

```text
FOLDER|Display Name|TEXT
FOLDER|Display Name|IMAGE
```

Text topic example:

```text
/RUSTMIX/APPS/FLASHCRD/TOPICS/BASICS/CARDS.TXT
```

`CARDS.TXT`:

```text
Q: Rustmix target?
A: Xteink X4
---
Q: SD app root?
A: /RUSTMIX/APPS
```

Image topic example:

```text
/RUSTMIX/APPS/FLASHCRD/TOPICS/IMGDEMO/CARDS.TXT
/RUSTMIX/APPS/FLASHCRD/TOPICS/IMGDEMO/IMG/IMG01F.X4B
/RUSTMIX/APPS/FLASHCRD/TOPICS/IMGDEMO/IMG/IMG01B.X4B
```

Convert images on your computer:

```bash
python3 tools/convert_flashcard_images.py input-images out-images
```

Copy the generated `.X4B` files into the topic `IMG` folder.

Controls:

```text
Topic list: Up/Down or Left/Right moves; OK opens; Back exits.
Inside topic: Up/Down or Left/Right changes card; OK flips; Back returns to topics.
```

## Add sleep images

Copy images to:

```text
/RUSTMIX/SLEEP
```

Valid file names:

```text
SLEEP.BMP
SLEEP00.BMP
SLEEP01.BMP
...
SLEEP32.BMP
```

Required image format:

```text
800 × 480
1-bit monochrome
uncompressed BMP
```

Convert images on your computer:

```bash
python3 tools/convert_sleep_images.py input-images out-sleep
```

Then copy generated BMP files into `/RUSTMIX/SLEEP`. Rustmix always selects randomly from that folder. No settings change is required.

## Manual SD-card starter pack

The repository contains an example SD card tree under:

```text
examples/sd-card/RUSTMIX
```

The release output also includes sleep images under:

```text
dist/rustmix-x4/sd-card/RUSTMIX/SLEEP
```
