#!/usr/bin/env bash
# Rebuild the site and run lychee over public/.
# Fails non-zero on any broken link so it can gate a commit.
#
# Usage:
#   scripts/check-links.sh              # offline: only internal links
#   scripts/check-links.sh --online     # also hit external URLs (slow, flaky)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

hugo --minify --gc --cleanDestinationDir

# 1) Dev-baseURL leak guard. Lychee's exclude list would silently
#    skip these; they are a deploy bug, not a known-bad external.
LEAKS=$(grep -rEln 'http://localhost:[0-9]+|http://127\.0\.0\.1' public/ \
          --include='*.html' --include='*.xml' || true)
if [[ -n "$LEAKS" ]]; then
  echo "dev baseURL leaked into public/. Rebuild with the production baseURL." >&2
  echo "$LEAKS" >&2
  exit 1
fi

# 2) Link check. --offline keeps it fast and deterministic. The
#    remap rewrites production URLs to local file paths so links
#    like https://sloppyquorum.com/about/ resolve against public/.
LYCHEE_FLAGS=(--offline)
if [[ "${1:-}" == "--online" ]]; then
  LYCHEE_FLAGS=()
fi

lychee \
  --config lychee.toml \
  --root-dir "$ROOT/public" \
  --remap "https://sloppyquorum.com/ file://$ROOT/public/" \
  "${LYCHEE_FLAGS[@]}" \
  './public/**/*.html' './public/**/*.xml'
