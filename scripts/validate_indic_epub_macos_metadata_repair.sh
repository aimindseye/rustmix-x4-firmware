#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cleanup() {
  rm -rf tools/font_prepare/__pycache__
}
trap cleanup EXIT

python3 -m py_compile tools/font_prepare/prepare_book.py

grep -q 'def is_macos_metadata' tools/font_prepare/prepare_book.py
grep -q 'def remove_macos_metadata' tools/font_prepare/prepare_book.py
grep -q 'remove_macos_metadata(cache_dir)' tools/font_prepare/prepare_book.py

python3 - "$ROOT" <<'PY'
from __future__ import annotations

import importlib.util
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

assert module.is_macos_metadata("._P00000.VRN")
assert module.is_macos_metadata("._META.TXT")
assert module.is_macos_metadata(".DS_Store")
assert not module.is_macos_metadata("P00000.VRN")
assert not module.is_macos_metadata("META.TXT")

with tempfile.TemporaryDirectory(prefix="rustmix-macos-metadata-test-") as td:
    cache = Path(td) / "FCACHE" / "FBB52E15"
    nested = cache / "nested"
    nested.mkdir(parents=True)
    (cache / "P00000.VRN").write_bytes(b"asset")
    (cache / "._P00000.VRN").write_bytes(b"appledouble")
    (cache / ".DS_Store").write_bytes(b"finder")
    (nested / "._D00000.VFN").write_bytes(b"appledouble")
    removed = module.remove_macos_metadata(cache)
    assert removed == 3, removed
    assert (cache / "P00000.VRN").exists()
    assert not (cache / "._P00000.VRN").exists()
    assert not (cache / ".DS_Store").exists()
    assert not (nested / "._D00000.VFN").exists()

print("macos-appledouble-cleanup-regression=passed")
PY

echo 'indic-epub-macos-metadata-repair=passed'
