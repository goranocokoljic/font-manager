#!/usr/bin/env bash
# Download the ~10,000-font reference library (OFL, Apache 2.0 and UFL only) that the
# performance budgets are measured against. Google Fonts repository at a pinned commit,
# the static instances fonts.google.com serves for its variable families, and every
# OFL family of Fontshare. Writes <dest>/manifest.tsv (sha256, size, licence, source,
# path). Idempotent and resumable: run it again after an interruption.
#
#   scripts/build-reference-library.sh [--dest DIR] [--sources ...] [--jobs N] [--limit N]
#   export FM_REFERENCE_LIBRARY=<dest>    # then the performance tests use it
#
# Options: node scripts/reference-library/build.mjs --help
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v node >/dev/null 2>&1; then
  echo "build-reference-library: Node.js 22 or newer is required" >&2
  exit 2
fi
major="$(node -p 'process.versions.node.split(".")[0]')"
if [ "$major" -lt 22 ]; then
  echo "build-reference-library: Node.js 22 or newer is required (found $(node --version))" >&2
  exit 2
fi

exec node "$here/reference-library/build.mjs" "$@"
