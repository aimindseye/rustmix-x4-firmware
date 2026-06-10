#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 /path/to/rustmix-x4-firmware" >&2
  exit 2
fi

OVERLAY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$1" && pwd)"

for required in README.md SCREENSHOTS.md screenshots; do
  if [ ! -e "$REPO_ROOT/$required" ]; then
    echo "expected repository path missing: $REPO_ROOT/$required" >&2
    exit 1
  fi
done

install -m 0644 "$OVERLAY_ROOT/files/README.md" "$REPO_ROOT/README.md"
install -m 0644 "$OVERLAY_ROOT/files/SCREENSHOTS.md" "$REPO_ROOT/SCREENSHOTS.md"
install -m 0644 "$OVERLAY_ROOT/files/screenshots/epub-gujarati-ramayana.jpg" "$REPO_ROOT/screenshots/epub-gujarati-ramayana.jpg"
install -m 0644 "$OVERLAY_ROOT/files/screenshots/epub-hindi-bhagavat.jpg" "$REPO_ROOT/screenshots/epub-hindi-bhagavat.jpg"
install -m 0644 "$OVERLAY_ROOT/files/screenshots/epub-hindi-garud-purana.jpg" "$REPO_ROOT/screenshots/epub-hindi-garud-purana.jpg"

echo "indic-epub-documentation-overlay=applied"
echo "repo=$REPO_ROOT"
