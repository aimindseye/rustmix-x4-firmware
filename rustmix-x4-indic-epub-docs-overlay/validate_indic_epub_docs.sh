#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 /path/to/rustmix-x4-firmware" >&2
  exit 2
fi

REPO_ROOT="$(cd "$1" && pwd)"
cd "$REPO_ROOT"

required_images=(
  screenshots/epub-gujarati-ramayana.jpg
  screenshots/epub-hindi-bhagavat.jpg
  screenshots/epub-hindi-garud-purana.jpg
)

for image in "${required_images[@]}"; do
  test -s "$image" || { echo "missing or empty screenshot: $image" >&2; exit 1; }
  grep -Fq "$image" README.md || { echo "README.md missing screenshot reference: $image" >&2; exit 1; }
  grep -Fq "$image" SCREENSHOTS.md || { echo "SCREENSHOTS.md missing screenshot reference: $image" >&2; exit 1; }
done

grep -Fq '## Hindi and Gujarati EPUB support' README.md
grep -Fq '/FCACHE                 # host-prepared EPUB caches; separate from /RUSTMIX' README.md
grep -Fq 'NotoSansDevanagari-Medium.ttf' README.md
grep -Fq 'NotoSansGujarati-Regular.ttf' README.md
if grep -Fq 'scripts/create_rustmix_release_package.sh' README.md; then
  echo 'README.md still references removed create_rustmix_release_package.sh' >&2
  exit 1
fi

python3 - <<'PY'
from pathlib import Path
import re
root = Path('.')
for doc in ('README.md', 'SCREENSHOTS.md'):
    text = (root / doc).read_text(encoding='utf-8')
    for target in re.findall(r'!?(?:\[[^]]*\])\(([^)#]+)(?:#[^)]+)?\)', text):
        if '://' in target or target.startswith('mailto:'):
            continue
        path = (root / target).resolve()
        if not path.exists():
            raise SystemExit(f'{doc}: missing local link target: {target}')
print('markdown-local-links=passed')
PY

./scripts/check_repo_hygiene.sh
./scripts/audit_remaining_pulp_runtime_dependencies.sh
./scripts/validate_x4_standard_partition_table_compatibility.sh
./scripts/validate_x4_flash_ota_slot_policy.sh

echo 'indic-epub-documentation=passed'
