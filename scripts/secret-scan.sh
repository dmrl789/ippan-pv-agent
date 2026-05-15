#!/usr/bin/env bash
# Tiny secret-scan covering committed evidence, config, and code fixtures.
# Documentation files are excluded — they may discuss these terms.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Patterns that must not appear in committed evidence/config/code fixtures.
PATTERNS=(
  'BEGIN PRIVATE KEY'
  '"admin_token"[[:space:]]*:'
  '"bearer"[[:space:]]*:'
  '"operator_private"[[:space:]]*:'
  '"private_key"[[:space:]]*:'
  'IPPAN_ADMIN_TOKEN[[:space:]]*='
)

# Paths to scan: generated evidence, committed config, and code fixtures.
# Test files are excluded because they contain negative assertions that
# legitimately reference these tokens to prove their absence elsewhere.
SCAN_PATHS=(
  "$ROOT/examples"
  "$ROOT/data"
)

failed=0
for path in "${SCAN_PATHS[@]}"; do
  [ -e "$path" ] || continue
  for pat in "${PATTERNS[@]}"; do
    if grep -RInE "$pat" "$path" 2>/dev/null; then
      echo "FAIL: pattern '$pat' found under $path" >&2
      failed=1
    fi
  done
done

if [ "$failed" -ne 0 ]; then
  exit 1
fi
echo "secret-scan: clean"
