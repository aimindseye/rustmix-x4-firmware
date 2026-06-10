# Rustmix Architecture

## Product model

Rustmix targets the Xteink X4 as a reader-first firmware. The home screen uses a simple category launcher, while internal screens use compact e-ink-friendly chrome with safe header/body/footer regions.

## Crates and runtime paths

```text
core/                 format-neutral models and contracts
hal-xteink-x4/        X4-facing hardware seams
target-xteink-x4/     production X4 firmware target
support/rustmix-lua-vm optional hostable Lua VM bridge
```

Active firmware behavior belongs under:

```text
target-xteink-x4/src/rustmix_x4/**
```

`vendor/pulp-os` may remain for reference or comparison. New product behavior should not be added there.

## Hardware target

```text
MCU:        ESP32-C3
Display:    4.26 inch 800 × 480 e-paper panel
Controller: SSD1677
Flash:      16 MB
Storage:    microSD on shared SPI bus
Input:      X4 hardware buttons
```

## Partition policy

Rustmix preserves the X4/CrossPoint-compatible layout:

```text
nvs       0x009000  0x005000
otadata   0x00e000  0x002000
app0      0x010000  0x640000
app1      0x650000  0x640000
spiffs    0xc90000  0x360000
coredump  0xff0000  0x010000
```

The release scripts and CI validators should protect this layout.

## First-boot SD provisioning

On boot, Rustmix calls the SD provisioning path after the SD card is available. Provisioning creates missing folders and files under `/RUSTMIX` and does not overwrite normal user data.

Required roots:

```text
/RUSTMIX
/RUSTMIX/APPS
/RUSTMIX/FONTS
/RUSTMIX/SLEEP
/RUSTMIX/CACHE
/RUSTMIX/STATE
```

The firmware embeds small starter payloads for default apps, fonts, dictionary shards, flashcard demos, and sleep images using `include_bytes!` from `examples/sd-card/RUSTMIX/**`.

## Lua app architecture

Lua apps are optional SD-loaded apps. Each app has an 8.3-safe folder under:

```text
/RUSTMIX/APPS/<APPID>
```

Typical layout:

```text
/RUSTMIX/APPS/MYAPP/APP.TOM
/RUSTMIX/APPS/MYAPP/MAIN.LUA
/RUSTMIX/APPS/MYAPP/DATA.TXT
```

`APP.TOM` describes the app. `MAIN.LUA` is the app entry file. App data should stay inside that app folder. Native firmware features remain native unless intentionally exposed through the bounded Lua host API.

## Custom font architecture

Custom fonts live under:

```text
/RUSTMIX/FONTS
```

The font folder contains VFN files plus a manifest:

```text
/RUSTMIX/FONTS/MANIFEST.TXT
/RUSTMIX/FONTS/INTER14.VFN
/RUSTMIX/FONTS/LEXEND18.VFN
```

Book fonts and UI fonts are separate. Reader/book font settings should not change OS chrome unless the user explicitly selects an SD UI font profile.

## Prepared Indic reader cache

Devanagari and Gujarati shaping stays host-side. `tools/font_prepare/prepare_book.py` extracts ordered EPUB spine text and calls the Rustybuzz/fontdue host generator. The X4 bridge in `reader/prepared_txt.rs` renders compact page-local assets from:

```text
/FCACHE/<BOOKID>/META.TXT
/FCACHE/<BOOKID>/P00000.VRN
/FCACHE/<BOOKID>/L00000.VFN
/FCACHE/<BOOKID>/D00000.VFN
/FCACHE/<BOOKID>/G00000.VFN   # optional
```

`cache_format=2` uses deterministic FAT 8.3-safe base36 page names and loads only the active page's script assets. This keeps whole-book Indic glyph inventories out of the ESP32-C3 heap. The bridge still accepts older global-font `FONTS.IDX` + `PAGES.IDX` caches for compatibility. Reader/book fonts remain isolated from OS chrome fonts.

## Dictionary architecture

The dictionary app uses a small prefix-shard model so the X4 does not need to load one large JSON file.

```text
/RUSTMIX/APPS/DICT/APP.TOM
/RUSTMIX/APPS/DICT/MAIN.LUA
/RUSTMIX/APPS/DICT/INDEX.TXT
/RUSTMIX/APPS/DICT/DICT.JSN
/RUSTMIX/APPS/DICT/DATA/A.JSN
/RUSTMIX/APPS/DICT/DATA/B.JSN
...
```

`INDEX.TXT` maps prefixes to shard files. `DICT.JSN` is a compact fallback. Shards use uppercase lookup keys and compact JSON entries.

## Flashcards architecture

Flashcards are topic-based. The app opens a topic list first. A topic declares whether it is text-based or image-based.

```text
/RUSTMIX/APPS/FLASHCRD/APP.TOM
/RUSTMIX/APPS/FLASHCRD/MAIN.LUA
/RUSTMIX/APPS/FLASHCRD/TOPICS/INDEX.TXT
/RUSTMIX/APPS/FLASHCRD/TOPICS/TEXTDEMO/CARDS.TXT
/RUSTMIX/APPS/FLASHCRD/TOPICS/IMGDEMO/CARDS.TXT
/RUSTMIX/APPS/FLASHCRD/TOPICS/IMGDEMO/IMG/*.X4B
```

Text topics store front/back text in `CARDS.TXT`. Image topics reference `.X4B` image cards converted for the X4 display.

## Sleep image architecture

Sleep images live under:

```text
/RUSTMIX/SLEEP
```

Sleep images are always selected randomly from:

```text
SLEEP.BMP
SLEEP00.BMP
SLEEP01.BMP ... SLEEP32.BMP
```

Images must be 800 × 480, 1-bit monochrome, uncompressed BMP files.

## Release artifact architecture

The source build emits an ELF. Release scripts create:

```text
dist/rustmix-x4/rustfirmware.bin
dist/rustmix-x4/README-RUSTFIRMWARE.txt
dist/rustmix-x4/SHA256SUMS.txt
dist/rustmix-x4/sd-card/RUSTMIX/SLEEP/*
```

`rustfirmware.bin` is the main firmware image. The SD-card pack is included for manual recovery or copying, but the firmware also seeds sleep images on boot when missing.


## Documentation architecture

Release documentation is intentionally kept in root-level Markdown files: `README.md`, `ARCHITECTURE.md`, `USERGUIDE.md`, `SCOPE.md`, `ROADMAP.md`, and `SCREENSHOTS.md`. The repository does not use a `docs/` directory for the first release because CI hygiene checks keep the release guidance consolidated and visible.
