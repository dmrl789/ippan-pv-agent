#!/usr/bin/env bash
# Secret-scan: refuses to ship if any file that git would track contains
# patterns that look like an accidentally-committed secret.
#
# Scope: this scan deliberately ignores anything covered by .gitignore.
# Local demo keys, evidence bundles, and other generated artifacts live in
# gitignored paths and MUST NOT trip the scan. The scan exists to catch
# real commit-time leaks, not to police a developer's local workspace.
#
# Test files are excluded: they contain negative assertions referencing
# these patterns to prove their absence elsewhere.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Patterns that must not appear inside files git would track.
PATTERNS=(
  'BEGIN PRIVATE KEY'
  '"admin_token"[[:space:]]*:'
  '"bearer"[[:space:]]*:'
  '"operator_private"[[:space:]]*:'
  '"private_key"[[:space:]]*:'
  '"secret_seed_b64"[[:space:]]*:'
  '"secret_key"[[:space:]]*:'
  'IPPAN_ADMIN_TOKEN[[:space:]]*='
)

# Files git would track or would notice as untracked-but-not-ignored,
# minus the tests/ directory (negative assertions) and docs/ (explanatory
# prose).
if command -v git >/dev/null 2>&1 && [ -d "$ROOT/.git" ]; then
  mapfile -t FILES < <(git ls-files --cached --others --exclude-standard \
    | grep -v -E '^(tests/|docs/|scripts/|\.git/)' \
    | grep -v -E '\.(md)$')
else
  # Fallback: scan examples + tracked-style paths directly.
  mapfile -t FILES < <(find examples src Cargo.toml -type f 2>/dev/null || true)
fi

failed=0
for f in "${FILES[@]}"; do
  [ -f "$f" ] || continue
  for pat in "${PATTERNS[@]}"; do
    if grep -InE "$pat" "$f" 2>/dev/null; then
      echo "FAIL: pattern '$pat' found in $f" >&2
      failed=1
    fi
  done
done

if [ "$failed" -ne 0 ]; then
  echo "secret-scan: FAILED — refusing to ship"
  exit 1
fi
echo "secret-scan: clean"
