#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cleanup() {
  rm -rf tools/font_prepare/__pycache__
}
trap cleanup EXIT

python3 -m py_compile tools/font_prepare/prepare_book.py

grep -q 'process = subprocess.Popen(' tools/font_prepare/prepare_book.py
grep -q 'stderr=subprocess.STDOUT' tools/font_prepare/prepare_book.py
grep -q 'output = "".join(output_lines)' tools/font_prepare/prepare_book.py
grep -q 'for line in output.splitlines()' tools/font_prepare/prepare_book.py
if grep -q 'completed.stdout.splitlines()' tools/font_prepare/prepare_book.py; then
  echo 'ERROR: stale completed.stdout parser remains' >&2
  exit 1
fi

python3 - "$ROOT" <<'PY'
from __future__ import annotations

import importlib.util
import os
import stat
import sys
import tempfile
from pathlib import Path

root = Path(sys.argv[1])
module_path = root / "tools/font_prepare/prepare_book.py"
spec = importlib.util.spec_from_file_location("rustmix_prepare_book", module_path)
assert spec and spec.loader
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

with tempfile.TemporaryDirectory(prefix="rustmix-generator-output-test-") as td:
    temp = Path(td)
    fake_bin = temp / "bin"
    fake_bin.mkdir()
    fake_cargo = fake_bin / "cargo"
    fake_cargo.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "echo cargo-stderr-visible >&2\n"
        "echo book_id=FBB52E15\n"
        "echo prepared cache: \"$RUSTMIX_FAKE_CACHE\"\n"
        "echo cache_format=2\n"
        "echo pages=290\n"
        "exit \"${RUSTMIX_FAKE_EXIT_CODE:-0}\"\n",
        encoding="utf-8",
    )
    fake_cargo.chmod(fake_cargo.stat().st_mode | stat.S_IXUSR)

    book = temp / "BOOK.TXT"
    latin = temp / "NotoSans-Regular.ttf"
    devanagari = temp / "NotoSansDevanagari-Regular.ttf"
    cache = temp / "FCACHE/FBB52E15"
    cache.mkdir(parents=True)
    book.write_text("नमस्ते\n", encoding="utf-8")
    latin.write_bytes(b"font")
    devanagari.write_bytes(b"font")

    old_path = os.environ.get("PATH", "")
    old_cache = os.environ.get("RUSTMIX_FAKE_CACHE")
    old_exit = os.environ.get("RUSTMIX_FAKE_EXIT_CODE")
    os.environ["PATH"] = str(fake_bin) + os.pathsep + old_path
    os.environ["RUSTMIX_FAKE_CACHE"] = str(cache)
    try:
        result = module.run_prepared_txt_generator(
            book=book,
            device_path="GARUDPUR.EPU",
            latin_font=latin,
            devanagari_font=devanagari,
            gujarati_font=None,
            out=temp / "FCACHE",
            title="book in hindi",
            latin_size=18,
            devanagari_size=22,
            gujarati_size=22,
            line_height=None,
            page_width=464,
            page_height=730,
            margin_x=0,
            margin_y=4,
        )
        assert result.book_id == "FBB52E15", result
        assert result.cache_dir == cache, result
        assert "cargo-stderr-visible" in result.stdout, result.stdout
        assert "pages=290" in result.stdout, result.stdout

        os.environ["RUSTMIX_FAKE_EXIT_CODE"] = "7"
        try:
            module.run_prepared_txt_generator(
                book=book,
                device_path="GARUDPUR.EPU",
                latin_font=latin,
                devanagari_font=devanagari,
                gujarati_font=None,
                out=temp / "FCACHE",
                title=None,
                latin_size=18,
                devanagari_size=22,
                gujarati_size=22,
                line_height=None,
                page_width=464,
                page_height=730,
                margin_x=0,
                margin_y=4,
            )
        except RuntimeError as exc:
            assert "exit status 7" in str(exc), exc
        else:
            raise AssertionError("non-zero fake cargo exit was not surfaced")
    finally:
        os.environ["PATH"] = old_path
        if old_cache is None:
            os.environ.pop("RUSTMIX_FAKE_CACHE", None)
        else:
            os.environ["RUSTMIX_FAKE_CACHE"] = old_cache
        if old_exit is None:
            os.environ.pop("RUSTMIX_FAKE_EXIT_CODE", None)
        else:
            os.environ["RUSTMIX_FAKE_EXIT_CODE"] = old_exit

print("generator-output-tee-regression=passed")
PY

echo 'indic-epub-generator-output-repair=passed'
