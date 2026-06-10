#!/usr/bin/env python3
"""Prepare host-shaped TXT and EPUB caches for the Rustmix X4 Reader.

The X4 intentionally does not run an Indic shaping engine on-device. This tool
extracts EPUB text, audits scripts, calls the Rustybuzz/fontdue host shaper, and
validates the emitted FAT 8.3-safe `/FCACHE/<BOOKID>/` cache before SD upload.
"""

from __future__ import annotations

import argparse
import html
import posixpath
import re
import shlex
import shutil
import struct
import subprocess
import sys
import tempfile
import urllib.parse
import zipfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from xml.etree import ElementTree as ET

DEFAULT_LATIN_FONT_NAMES = [
    "NotoSans-Regular.ttf",
    "NotoSans-Medium.ttf",
]
DEFAULT_DEVANAGARI_FONT_NAMES = [
    "NotoSansDevanagari-Regular.ttf",
    "NotoSansDevanagari-Medium.ttf",
]
DEFAULT_GUJARATI_FONT_NAMES = [
    "NotoSansGujarati-Regular.ttf",
    "NotoSansGujarati-Medium.ttf",
]

CACHE_FORMAT_PAGE_LOCAL = 2
PAGE_LOCAL_DIGITS = 5
PAGE_LOCAL_CAPACITY = 36**PAGE_LOCAL_DIGITS
FIRMWARE_MAX_PAGE_BYTES = 24 * 1024
FIRMWARE_MAX_FONT_BYTES = 24 * 1024
FIRMWARE_MAX_PAGE_GLYPHS = 1024
VFNT_HEADER_LEN = 44
VFNT_METRICS_LEN = 16
VFNT_BITMAP_INDEX_LEN = 16
VRUN_HEADER_LEN = 20
VRUN_RECORD_LEN = 20

FONT_PREFIX = {1: "L", 2: "D", 3: "G"}
FONT_LABEL = {1: "Latin", 2: "Devanagari", 3: "Gujarati"}

DEVANAGARI_RANGES = (
    (0x0900, 0x097F),
    (0x1CD0, 0x1CFF),  # Vedic Extensions
    (0xA8E0, 0xA8FF),  # Devanagari Extended
    (0x11B00, 0x11B5F),  # Devanagari Extended-A
)
GUJARATI_RANGES = ((0x0A80, 0x0AFF),)


@dataclass
class PreparedResult:
    book_id: str | None
    cache_dir: Path | None
    stdout: str


class TextExtractor(HTMLParser):
    BLOCK_TAGS = {
        "p",
        "div",
        "section",
        "article",
        "br",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "li",
        "tr",
        "blockquote",
        "pre",
    }

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.parts: list[str] = []
        self.skip_depth = 0

    def handle_starttag(self, tag: str, attrs) -> None:
        del attrs
        tag = tag.lower()
        if tag in {"script", "style", "svg"}:
            self.skip_depth += 1
        elif tag in self.BLOCK_TAGS:
            self._newline()

    def handle_endtag(self, tag: str) -> None:
        tag = tag.lower()
        if tag in {"script", "style", "svg"} and self.skip_depth:
            self.skip_depth -= 1
        elif tag in self.BLOCK_TAGS:
            self._newline()

    def handle_data(self, data: str) -> None:
        if self.skip_depth:
            return
        data = re.sub(r"\s+", " ", data)
        if data.strip():
            self.parts.append(data)

    def _newline(self) -> None:
        if self.parts and not self.parts[-1].endswith("\n"):
            self.parts.append("\n")

    def text(self) -> str:
        raw = html.unescape("".join(self.parts))
        raw = re.sub(r"[ \t]+\n", "\n", raw)
        raw = re.sub(r"\n{3,}", "\n\n", raw)
        return raw.strip()


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def ns_tag(name: str) -> str:
    return name.split("}", 1)[-1]


def resolve_opf_path(opf_path: str, href: str) -> str:
    clean = urllib.parse.unquote(urllib.parse.urlsplit(href).path)
    return posixpath.normpath(posixpath.join(posixpath.dirname(opf_path), clean))


def find_container_opf(zf: zipfile.ZipFile) -> str:
    root = ET.fromstring(zf.read("META-INF/container.xml"))
    for elem in root.iter():
        if ns_tag(elem.tag) == "rootfile":
            path = elem.attrib.get("full-path")
            if path:
                return path
    raise ValueError("container.xml has no rootfile full-path")


def extract_epub_text(epub: Path) -> tuple[str, str]:
    with zipfile.ZipFile(epub) as zf:
        opf_path = find_container_opf(zf)
        opf_root = ET.fromstring(zf.read(opf_path))

        title = ""
        for elem in opf_root.iter():
            if ns_tag(elem.tag) == "title" and elem.text and elem.text.strip():
                title = elem.text.strip()
                break

        manifest: dict[str, tuple[str, str]] = {}
        for elem in opf_root.iter():
            if ns_tag(elem.tag) != "item":
                continue
            item_id = elem.attrib.get("id")
            href = elem.attrib.get("href")
            media_type = elem.attrib.get("media-type", "")
            if item_id and href:
                manifest[item_id] = (resolve_opf_path(opf_path, href), media_type)

        spine_ids = [
            elem.attrib["idref"]
            for elem in opf_root.iter()
            if ns_tag(elem.tag) == "itemref" and elem.attrib.get("idref")
        ]

        chunks: list[str] = []
        for item_id in spine_ids:
            entry = manifest.get(item_id)
            if not entry:
                continue
            href, media_type = entry
            if "html" not in media_type and not href.lower().endswith((".xhtml", ".html", ".htm")):
                continue
            try:
                data = zf.read(href)
            except KeyError:
                continue
            parser = TextExtractor()
            parser.feed(data.decode("utf-8", errors="replace"))
            text = parser.text()
            if text:
                chunks.append(text)

    if not chunks:
        raise ValueError("EPUB spine produced no extractable text")
    return "\n\n".join(chunks) + "\n", title or epub.stem


def in_ranges(ch: str, ranges: tuple[tuple[int, int], ...]) -> bool:
    codepoint = ord(ch)
    return any(start <= codepoint <= end for start, end in ranges)


def audit_text(text: str) -> Counter[str]:
    counts: Counter[str] = Counter()
    for ch in text:
        if in_ranges(ch, DEVANAGARI_RANGES):
            counts["devanagari"] += 1
        elif in_ranges(ch, GUJARATI_RANGES):
            counts["gujarati"] += 1
        elif ch.isspace():
            counts["whitespace"] += 1
        elif ch.isascii():
            counts["latin_ascii"] += 1
        else:
            counts["other"] += 1
    counts["characters"] = len(text)
    counts["utf8_bytes"] = len(text.encode("utf-8"))
    counts["lines"] = text.count("\n") + (1 if text else 0)
    return counts


def print_script_audit(label: str, counts: Counter[str]) -> None:
    print(f"script_audit={label}")
    for key in ("characters", "utf8_bytes", "lines", "devanagari", "gujarati", "latin_ascii", "whitespace", "other"):
        print(f"  {key}={counts.get(key, 0)}")


def required_scripts(counts: Counter[str]) -> set[str]:
    scripts = {"latin", "devanagari"}
    if counts.get("gujarati", 0):
        scripts.add("gujarati")
    return scripts


def find_font(
    fonts_dir: Path | None,
    explicit: Path | None,
    names: list[str],
    label: str,
    *,
    required: bool,
) -> Path | None:
    if explicit:
        if explicit.exists():
            return explicit
        raise FileNotFoundError(f"{label} font does not exist: {explicit}")
    if not fonts_dir:
        if required:
            raise FileNotFoundError(f"{label} font not provided. Use --{label}-font or --fonts-dir.")
        return None
    candidates: list[Path] = []
    for name in names:
        candidates.extend(fonts_dir.rglob(name))
    if candidates:
        candidates.sort(key=lambda path: (len(str(path)), str(path)))
        return candidates[0]
    if required:
        expected = ", ".join(names)
        raise FileNotFoundError(f"Could not find {label} font under {fonts_dir}. Expected one of: {expected}")
    return None


def require_static_font(path: Path, label: str) -> Path:
    """Reject variable-font files before the Rust raster stage.

    Rustybuzz can shape variable fonts, but the cache generator intentionally
    keeps the raster path deterministic by consuming one static TTF face per
    script. Google Fonts downloads often install variable files by default, so
    fail early with an actionable message instead of surfacing a parser error
    inside Cargo.
    """

    lowered = path.name.lower()
    variable_markers = ("variablefont", "[wght]", "[wdth]", "-vf.", "-vf_")
    if any(marker in lowered for marker in variable_markers):
        raise ValueError(
            f"{label} font must be a static TTF face, not a variable font: {path}. "
            f"Install or pass a static file such as {label_static_example(label)}."
        )
    return path


def label_static_example(label: str) -> str:
    return {
        "latin": "NotoSans-Regular.ttf",
        "devanagari": "NotoSansDevanagari-Regular.ttf",
        "gujarati": "NotoSansGujarati-Regular.ttf",
    }[label]


def prepared_txt_manifest() -> Path:
    manifest = repo_root() / "tools/prepared_txt_real_vfnt/Cargo.toml"
    if not manifest.exists():
        raise FileNotFoundError(f"missing prepared TXT generator: {manifest}")
    return manifest


def clean_generated_artifacts() -> None:
    for path in (
        repo_root() / "tools/prepared_txt_real_vfnt/target",
        repo_root() / "tools/prepared_epub_smoke/__pycache__",
        repo_root() / "tools/font_prepare/__pycache__",
    ):
        if path.exists():
            print(f"removing generated artifact: {path}")
            shutil.rmtree(path, ignore_errors=True)


def run_prepared_txt_generator(
    *,
    book: Path,
    device_path: str,
    latin_font: Path,
    devanagari_font: Path,
    gujarati_font: Path | None,
    out: Path,
    title: str | None,
    latin_size: int,
    devanagari_size: int,
    gujarati_size: int,
    line_height: int | None,
    page_width: int,
    page_height: int,
    margin_x: int,
    margin_y: int,
) -> PreparedResult:
    cmd = [
        "cargo",
        "run",
        "--manifest-path",
        str(prepared_txt_manifest()),
        "--release",
        "--",
        "--book",
        str(book),
        "--device-path",
        device_path,
        "--latin-font",
        str(latin_font),
        "--devanagari-font",
        str(devanagari_font),
        "--out",
        str(out),
        "--latin-size",
        str(latin_size),
        "--devanagari-size",
        str(devanagari_size),
        "--gujarati-size",
        str(gujarati_size),
        "--page-width",
        str(page_width),
        "--page-height",
        str(page_height),
        "--margin-x",
        str(margin_x),
        "--margin-y",
        str(margin_y),
    ]
    if gujarati_font:
        cmd.extend(["--gujarati-font", str(gujarati_font)])
    if title:
        cmd.extend(["--title", title])
    if line_height:
        cmd.extend(["--line-height", str(line_height)])

    print("running prepared cache generator:", flush=True)
    print(shlex.join(str(item) for item in cmd), flush=True)
    print("generator_output=begin", flush=True)
    completed = subprocess.run(cmd, text=True)
    print("generator_output=end", flush=True)
    if completed.returncode != 0:
        raise RuntimeError(
            f"prepared cache generator failed with exit status {completed.returncode}; "
            "review the Cargo/Rust error printed above"
        )

    book_id = None
    cache_dir = None
    for line in completed.stdout.splitlines():
        if line.startswith("book_id="):
            book_id = line.split("=", 1)[1].strip()
        elif line.startswith("prepared cache: "):
            cache_dir = Path(line.split(": ", 1)[1].strip())
    if book_id and cache_dir is None:
        cache_dir = out / book_id
    return PreparedResult(book_id=book_id, cache_dir=cache_dir, stdout=completed.stdout)


def base36(value: int, digits: int = PAGE_LOCAL_DIGITS) -> str:
    if not 0 <= value < 36**digits:
        raise ValueError(f"page index outside base36 range: {value}")
    alphabet = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    out = ["0"] * digits
    for index in range(digits - 1, -1, -1):
        out[index] = alphabet[value % 36]
        value //= 36
    return "".join(out)


def page_file(prefix: str, page: int, extension: str) -> str:
    return f"{prefix}{base36(page)}.{extension}"


def is_fat_83(name: str) -> bool:
    match = re.fullmatch(r"([A-Za-z0-9_]{1,8})(?:\.([A-Za-z0-9_]{1,3}))?", name)
    return bool(match)


def parse_meta(cache_dir: Path) -> dict[str, str]:
    meta = cache_dir / "META.TXT"
    if not meta.exists():
        raise ValueError("missing META.TXT")
    values: dict[str, str] = {}
    for raw in meta.read_text(encoding="utf-8", errors="replace").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip()
    return values


def read_u16(data: bytes, offset: int) -> int:
    if offset + 2 > len(data):
        raise ValueError("read past end of binary asset")
    return struct.unpack_from("<H", data, offset)[0]


def read_u32(data: bytes, offset: int) -> int:
    if offset + 4 > len(data):
        raise ValueError("read past end of binary asset")
    return struct.unpack_from("<I", data, offset)[0]


def checked_range(total: int, start: int, length: int, label: str) -> None:
    if start < 0 or length < 0 or start + length > total:
        raise ValueError(f"{label} range outside asset")


def validate_vfnt(path: Path) -> set[int]:
    data = path.read_bytes()
    if len(data) > FIRMWARE_MAX_FONT_BYTES:
        raise ValueError(f"{path.name} is {len(data)} bytes; firmware font limit is {FIRMWARE_MAX_FONT_BYTES}")
    if len(data) < VFNT_HEADER_LEN or data[0:4] != b"VFNT":
        raise ValueError(f"{path.name} has invalid VFNT header")
    if read_u16(data, 4) != 1 or read_u16(data, 42) != 1:
        raise ValueError(f"{path.name} has unsupported VFNT version or bitmap format")
    count = read_u32(data, 20)
    metrics_offset = read_u32(data, 24)
    bitmap_index_offset = read_u32(data, 28)
    bitmap_data_offset = read_u32(data, 32)
    bitmap_data_len = read_u32(data, 36)
    checked_range(len(data), metrics_offset, count * VFNT_METRICS_LEN, f"{path.name} metrics")
    checked_range(len(data), bitmap_index_offset, count * VFNT_BITMAP_INDEX_LEN, f"{path.name} bitmap index")
    checked_range(len(data), bitmap_data_offset, bitmap_data_len, f"{path.name} bitmap data")
    ids: set[int] = set()
    for index in range(count):
        metrics = metrics_offset + index * VFNT_METRICS_LEN
        bitmap = bitmap_index_offset + index * VFNT_BITMAP_INDEX_LEN
        glyph_id = read_u32(data, metrics)
        if read_u32(data, bitmap) != glyph_id:
            raise ValueError(f"{path.name} metrics/bitmap glyph mismatch")
        checked_range(bitmap_data_len, read_u32(data, bitmap + 4), read_u32(data, bitmap + 8), f"{path.name} glyph")
        ids.add(glyph_id)
    if not ids:
        raise ValueError(f"{path.name} contains no glyphs")
    return ids


def validate_vrun(path: Path) -> list[tuple[int, int]]:
    data = path.read_bytes()
    if len(data) > FIRMWARE_MAX_PAGE_BYTES:
        raise ValueError(f"{path.name} is {len(data)} bytes; firmware page limit is {FIRMWARE_MAX_PAGE_BYTES}")
    if len(data) < VRUN_HEADER_LEN or data[0:4] != b"VRUN":
        raise ValueError(f"{path.name} has invalid VRUN header")
    if read_u16(data, 4) != 1 or read_u16(data, 6) < VRUN_HEADER_LEN:
        raise ValueError(f"{path.name} has unsupported VRUN version")
    count = read_u32(data, 8)
    if count > FIRMWARE_MAX_PAGE_GLYPHS:
        raise ValueError(f"{path.name} has {count} glyphs; firmware limit is {FIRMWARE_MAX_PAGE_GLYPHS}")
    checked_range(len(data), VRUN_HEADER_LEN, count * VRUN_RECORD_LEN, f"{path.name} records")
    refs: list[tuple[int, int]] = []
    for index in range(count):
        offset = VRUN_HEADER_LEN + index * VRUN_RECORD_LEN
        refs.append((read_u32(data, offset), read_u32(data, offset + 4)))
    return refs


def validate_page_local_cache(cache_dir: Path, meta: dict[str, str]) -> int:
    page_count = int(meta.get("page_count", "0"))
    if not 0 < page_count <= PAGE_LOCAL_CAPACITY:
        raise ValueError(f"invalid page_count={page_count}")
    largest_page = ("", 0)
    largest_font = ("", 0)
    script_pages: Counter[str] = Counter()
    for page in range(page_count):
        page_name = page_file("P", page, "VRN")
        page_path = cache_dir / page_name
        if not page_path.exists():
            raise ValueError(f"missing page asset: {page_name}")
        refs = validate_vrun(page_path)
        largest_page = max(largest_page, (page_name, page_path.stat().st_size), key=lambda item: item[1])
        refs_by_font: dict[int, set[int]] = defaultdict(set)
        for font_id, glyph_id in refs:
            if font_id not in FONT_PREFIX:
                raise ValueError(f"{page_name} references unknown font slot {font_id}")
            refs_by_font[font_id].add(glyph_id)
        for font_id, glyph_ids in refs_by_font.items():
            font_name = page_file(FONT_PREFIX[font_id], page, "VFN")
            font_path = cache_dir / font_name
            if not font_path.exists():
                raise ValueError(f"{page_name} requires missing page font: {font_name}")
            available = validate_vfnt(font_path)
            missing = glyph_ids - available
            if missing:
                raise ValueError(f"{page_name} references {len(missing)} glyph(s) absent from {font_name}")
            script_pages[FONT_LABEL[font_id]] += 1
            largest_font = max(largest_font, (font_name, font_path.stat().st_size), key=lambda item: item[1])
    print(f"OK: page-local cache format={CACHE_FORMAT_PAGE_LOCAL} pages={page_count}")
    print(f"OK: largest page asset={largest_page[0]} bytes={largest_page[1]}")
    print(f"OK: largest page font={largest_font[0]} bytes={largest_font[1]}")
    print("OK: script pages " + " ".join(f"{name}={script_pages.get(name, 0)}" for name in ("Latin", "Devanagari", "Gujarati")))
    return 0


def validate_legacy_cache(cache_dir: Path) -> int:
    fonts_idx = cache_dir / "FONTS.IDX"
    pages_idx = cache_dir / "PAGES.IDX"
    if not fonts_idx.exists() or not pages_idx.exists():
        raise ValueError("legacy cache requires FONTS.IDX and PAGES.IDX")
    fonts: dict[str, str] = {}
    for raw in fonts_idx.read_text(encoding="utf-8", errors="replace").splitlines():
        if "=" in raw:
            key, value = raw.split("=", 1)
            fonts[key.strip()] = value.strip()
    ids_by_label = {label: validate_vfnt(cache_dir / name) for label, name in fonts.items()}
    pages = [line.strip() for line in pages_idx.read_text(encoding="utf-8", errors="replace").splitlines() if line.strip()]
    if not pages:
        raise ValueError("legacy PAGES.IDX contains no pages")
    for page in pages:
        refs = validate_vrun(cache_dir / page)
        for font_id, glyph_id in refs:
            label = FONT_LABEL.get(font_id)
            if not label or glyph_id not in ids_by_label.get(label, set()):
                raise ValueError(f"{page} references missing glyph {glyph_id} in slot {font_id}")
    print(f"OK: legacy cache pages={len(pages)}")
    return 0


def source_matches(cache_source: str, device_path: str) -> bool:
    cache_source = cache_source.strip().lstrip("/\\")
    device_path = device_path.strip().lstrip("/\\")
    return cache_source.lower() == device_path.lower() or Path(cache_source).name.lower() == Path(device_path).name.lower()


def validate_cache(cache_dir: Path, device_path: str | None) -> int:
    print(f"validating cache={cache_dir}")
    if not cache_dir.is_dir():
        raise ValueError(f"cache directory does not exist: {cache_dir}")
    for path in cache_dir.iterdir():
        if path.is_file() and not is_fat_83(path.name):
            raise ValueError(f"cache filename is not FAT 8.3-safe: {path.name}")
    meta = parse_meta(cache_dir)
    if device_path and meta.get("source") and not source_matches(meta["source"], device_path):
        raise ValueError(f"META source={meta['source']} does not match device path={device_path}")
    cache_format = int(meta.get("cache_format", "1"))
    if cache_format == CACHE_FORMAT_PAGE_LOCAL:
        return validate_page_local_cache(cache_dir, meta)
    if cache_format == 1:
        return validate_legacy_cache(cache_dir)
    raise ValueError(f"unsupported cache_format={cache_format}")


def print_upload_instructions(cache_dir: Path | None, book_id: str | None) -> None:
    if not cache_dir or not book_id:
        print("WARN: generator output did not expose a cache directory")
        return
    files = sorted(path.name for path in cache_dir.iterdir() if path.is_file())
    print()
    print("SD upload:")
    print(f"  Copy the generated directory to /FCACHE/{book_id}/")
    print(f"  Keep the original TXT/EPUB at the exact --device-path used during preparation.")
    print(f"  Generated files={len(files)}")
    for name in files[:24]:
        print(f"    {name}")
    if len(files) > 24:
        print(f"    ... {len(files) - 24} more page-local assets")


def resolve_fonts(args: argparse.Namespace, counts: Counter[str]) -> tuple[Path, Path, Path | None]:
    latin = find_font(args.fonts_dir, args.latin_font, DEFAULT_LATIN_FONT_NAMES, "latin", required=True)
    devanagari = find_font(args.fonts_dir, args.devanagari_font, DEFAULT_DEVANAGARI_FONT_NAMES, "devanagari", required=True)
    needs_gujarati = "gujarati" in required_scripts(counts)
    gujarati = find_font(
        args.fonts_dir,
        args.gujarati_font,
        DEFAULT_GUJARATI_FONT_NAMES,
        "gujarati",
        required=True,
    ) if needs_gujarati else None
    assert latin is not None and devanagari is not None
    latin = require_static_font(latin, "latin")
    devanagari = require_static_font(devanagari, "devanagari")
    if gujarati:
        gujarati = require_static_font(gujarati, "gujarati")
    print(f"latin_font={latin}")
    print(f"devanagari_font={devanagari}")
    if gujarati:
        print(f"gujarati_font={gujarati}")
    else:
        print("gujarati_font=not-required")
    return latin, devanagari, gujarati


def run_prepare(args: argparse.Namespace, text_path: Path, text: str, title: str | None) -> int:
    counts = audit_text(text)
    print_script_audit(text_path.name, counts)
    latin, devanagari, gujarati = resolve_fonts(args, counts)
    result = run_prepared_txt_generator(
        book=text_path,
        device_path=args.device_path,
        latin_font=latin,
        devanagari_font=devanagari,
        gujarati_font=gujarati,
        out=args.out,
        title=title,
        latin_size=args.latin_size,
        devanagari_size=args.devanagari_size,
        gujarati_size=args.gujarati_size,
        line_height=args.line_height,
        page_width=args.page_width,
        page_height=args.page_height,
        margin_x=args.margin_x,
        margin_y=args.margin_y,
    )
    print_upload_instructions(result.cache_dir, result.book_id)
    if result.cache_dir:
        return validate_cache(result.cache_dir, args.device_path)
    return 0


def command_txt(args: argparse.Namespace) -> int:
    if args.clean:
        clean_generated_artifacts()
    text = args.book.read_text(encoding="utf-8")
    return run_prepare(args, args.book, text, args.title or args.book.stem)


def command_epub(args: argparse.Namespace) -> int:
    if args.clean:
        clean_generated_artifacts()
    text, detected_title = extract_epub_text(args.book)
    work_dir = Path(tempfile.mkdtemp(prefix="rustmix-font-epub-"))
    txt_path = work_dir / "BOOK.TXT"
    txt_path.write_text(text, encoding="utf-8")
    print(f"extracted_text={txt_path}")
    succeeded = False
    try:
        result = run_prepare(args, txt_path, text, args.title or detected_title)
        succeeded = True
        return result
    finally:
        if args.keep_work or not succeeded:
            print(f"work_dir={work_dir}")
            if not succeeded:
                print("work_dir_preserved=preparation-failed")
        else:
            shutil.rmtree(work_dir, ignore_errors=True)


def command_audit(args: argparse.Namespace) -> int:
    if args.book.suffix.lower() in {".epub", ".epu"}:
        text, title = extract_epub_text(args.book)
        print(f"title={title}")
    else:
        text = args.book.read_text(encoding="utf-8")
    print_script_audit(args.book.name, audit_text(text))
    return 0


def command_validate(args: argparse.Namespace) -> int:
    return validate_cache(args.cache, args.device_path)


def command_clean(args: argparse.Namespace) -> int:
    del args
    clean_generated_artifacts()
    return 0


def add_common_prepare_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--device-path", required=True, help="Exact filename/path as seen by X4")
    parser.add_argument("--fonts-dir", type=Path, help="Directory searched recursively for Noto fonts")
    parser.add_argument("--latin-font", type=Path, help="Path to NotoSans-Regular.ttf")
    parser.add_argument("--devanagari-font", type=Path, help="Path to NotoSansDevanagari-Regular.ttf")
    parser.add_argument("--gujarati-font", type=Path, help="Path to NotoSansGujarati-Regular.ttf when needed")
    parser.add_argument("--out", required=True, type=Path, help="Output FCACHE directory, e.g. /tmp/FCACHE")
    parser.add_argument("--title")
    parser.add_argument("--latin-size", type=int, default=18)
    parser.add_argument("--devanagari-size", type=int, default=22)
    parser.add_argument("--gujarati-size", type=int, default=22)
    parser.add_argument("--line-height", type=int)
    parser.add_argument("--page-width", type=int, default=464)
    parser.add_argument("--page-height", type=int, default=730)
    parser.add_argument("--margin-x", type=int, default=0)
    parser.add_argument("--margin-y", type=int, default=4)
    parser.add_argument("--clean", action="store_true", help="Remove generated local target/cache artifacts first")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Prepare Rustmix TXT/EPUB Indic caches for X4 Reader")
    sub = parser.add_subparsers(dest="command", required=True)

    txt = sub.add_parser("txt", help="Prepare a UTF-8 TXT book")
    txt.add_argument("--book", required=True, type=Path)
    add_common_prepare_args(txt)
    txt.set_defaults(func=command_txt)

    epub = sub.add_parser("epub", help="Extract and prepare an EPUB book")
    epub.add_argument("--book", required=True, type=Path)
    epub.add_argument("--keep-work", action="store_true")
    add_common_prepare_args(epub)
    epub.set_defaults(func=command_epub)

    audit = sub.add_parser("audit", help="Report Devanagari/Gujarati counts without generating a cache")
    audit.add_argument("--book", required=True, type=Path)
    audit.set_defaults(func=command_audit)

    validate = sub.add_parser("validate", help="Validate a generated /FCACHE/<BOOKID> directory")
    validate.add_argument("--cache", required=True, type=Path)
    validate.add_argument("--device-path")
    validate.set_defaults(func=command_validate)

    clean = sub.add_parser("clean", help="Remove generated tool target/cache artifacts")
    clean.set_defaults(func=command_clean)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        return args.func(args)
    except Exception as exc:  # CLI boundary: keep one actionable error line.
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
