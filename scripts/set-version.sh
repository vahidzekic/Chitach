#!/usr/bin/env sh
# Thin wrapper: scripts/set-version.sh v1.2.3
set -eu
exec node "$(dirname "$0")/set-version.mjs" "$@"
