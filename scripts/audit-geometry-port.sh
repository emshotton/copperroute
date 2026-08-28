#!/usr/bin/env bash
# Thin wrapper preserved for anyone still typing the Plan 1 script name; the real logic now
# lives in the generalised `audit-port.sh` (Plan 2 Task 1), which takes the Java subpath and
# Rust crate src dir as arguments instead of hardcoding fr-geometry.
set -euo pipefail
exec "$(cd "$(dirname "$0")" && pwd)/audit-port.sh" geometry/planar crates/fr-geometry/src
