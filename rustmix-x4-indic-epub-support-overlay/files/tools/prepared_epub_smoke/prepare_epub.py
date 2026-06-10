#!/usr/bin/env python3
"""Compatibility wrapper for the production Indic EPUB cache workflow.

New work should call `tools/font_prepare/prepare_book.py epub` directly. This
wrapper keeps the earlier smoke command usable while routing through the single
production extractor, audit, generator, and validator implementation.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


def run() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--epub", required=True, type=Path)
    parser.add_argument("--device-path", required=True)
    parser.add_argument("--fonts-dir", type=Path)
    parser.add_argument("--latin-font", type=Path)
    parser.add_argument("--devanagari-font", type=Path)
    parser.add_argument("--gujarati-font", type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--title")
    parser.add_argument("--latin-size", default="18")
    parser.add_argument("--devanagari-size", default="22")
    parser.add_argument("--gujarati-size", default="22")
    parser.add_argument("--line-height")
    parser.add_argument("--page-width", default="464")
    parser.add_argument("--page-height", default="730")
    parser.add_argument("--margin-x", default="0")
    parser.add_argument("--margin-y", default="4")
    parser.add_argument("--keep-work", action="store_true")
    parser.add_argument("--clean", action="store_true")
    args = parser.parse_args()

    production = Path(__file__).resolve().parents[1] / "font_prepare/prepare_book.py"
    cmd = [
        sys.executable,
        str(production),
        "epub",
        "--book",
        str(args.epub),
        "--device-path",
        args.device_path,
        "--out",
        str(args.out),
        "--latin-size",
        str(args.latin_size),
        "--devanagari-size",
        str(args.devanagari_size),
        "--gujarati-size",
        str(args.gujarati_size),
        "--page-width",
        str(args.page_width),
        "--page-height",
        str(args.page_height),
        "--margin-x",
        str(args.margin_x),
        "--margin-y",
        str(args.margin_y),
    ]
    for flag, value in (
        ("--fonts-dir", args.fonts_dir),
        ("--latin-font", args.latin_font),
        ("--devanagari-font", args.devanagari_font),
        ("--gujarati-font", args.gujarati_font),
        ("--title", args.title),
        ("--line-height", args.line_height),
    ):
        if value is not None:
            cmd.extend([flag, str(value)])
    if args.keep_work:
        cmd.append("--keep-work")
    if args.clean:
        cmd.append("--clean")
    return subprocess.run(cmd, check=False).returncode


if __name__ == "__main__":
    raise SystemExit(run())
