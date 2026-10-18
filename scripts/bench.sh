#!/usr/bin/env bash
# Measure the performance budgets on this machine against the reference library:
# cold start, first scan, incremental update, search latency, 100-face activation
# and idle memory. Writes a JSON report (default target/bench/bench-<os>.json) and
# prints a table. The library is only read; the index goes to a throwaway folder.
#
#   scripts/bench.sh [--library DIR] [--out FILE] [--runs N] [--rounds N] [--settle-ms N]
#   FM_REFERENCE_LIBRARY=<dir> scripts/bench.sh       # scripts/build-reference-library.sh makes it
#
# Exit code: 0 within budget, 1 a metric over budget, 2 usage or setup error.
# Options and the merge/trend commands CI uses: node scripts/bench/bench.mjs --help
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v node >/dev/null 2>&1; then
  echo "bench: Node.js 22 or newer is required" >&2
  exit 2
fi
major="$(node -p 'process.versions.node.split(".")[0]')"
if [ "$major" -lt 22 ]; then
  echo "bench: Node.js 22 or newer is required (found $(node --version))" >&2
  exit 2
fi

exec node "$here/bench/bench.mjs" run "$@"
