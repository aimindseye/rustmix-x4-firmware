#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
shift || true
ROOT="$(cd "$ROOT" && pwd)"
cd "$ROOT"

scripts/validate_prepared_indic_epub_support.sh "$@"
scripts/check_repo_hygiene.sh
scripts/audit_remaining_pulp_runtime_dependencies.sh
scripts/validate_x4_standard_partition_table_compatibility.sh
scripts/deploy/check_deploy_ready.sh

echo "rustmix-x4-indic-epub-overlay-validation=passed"
