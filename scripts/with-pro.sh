#!/usr/bin/env bash
# Chitach Pro build: puts the Pro code from the private repo into this checkout, then runs
# the given command (see docs/11-free-i-pro.md).
#
#   scripts/with-pro.sh [--link] [--] <command...>
#   scripts/with-pro.sh npm --prefix ui run tauri -- build --features pro
#   scripts/with-pro.sh cargo clippy --workspace --all-targets --features chitach/pro
#
# The private repo (vahidzekic/chitach-pro) is looked for in $CHITACH_PRO_DIR, else ./pro,
# else ../chitach-pro. It holds the Pro paths below at the same places as this repo; they
# are copied here (--link: symlinked, for working on Pro locally; not on Windows) and are
# ignored by git. Without the private repo nothing is copied and the build is Chitach
# Free. The command runs with CHITACH_PRO=1 when Pro is in place.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
# Paths owned by the private repo (keep in sync with .gitignore and docs/11-free-i-pro.md).
PRO_PATHS=(
  src-tauri/src/pro
  crates/celik-sign
  ui/src/lib/pro
)

link=0
if [ "${1:-}" = "--link" ]; then
  link=1
  shift
fi
if [ "${1:-}" = "--" ]; then
  shift
fi

pro_dir="${CHITACH_PRO_DIR:-}"
if [ -z "$pro_dir" ]; then
  for d in "$root/pro" "$root/../chitach-pro"; do
    if [ -d "$d/src-tauri/src/pro" ]; then
      pro_dir="$d"
      break
    fi
  done
fi

if [ -n "$pro_dir" ] && [ "$(cd "$pro_dir" && pwd)" != "$root" ]; then
  pro_dir="$(cd "$pro_dir" && pwd)"
  echo "Chitach Pro: $pro_dir"
  for p in "${PRO_PATHS[@]}"; do
    if [ ! -e "$pro_dir/$p" ]; then
      echo "::error::$pro_dir/$p is missing" >&2
      exit 1
    fi
    if [ "$link" = 1 ]; then
      rm -rf "${root:?}/$p"
      ln -s "$pro_dir/$p" "$root/$p"
    else
      # Copy over the public stand-ins (e.g. the manifest of crates/celik-sign).
      mkdir -p "$root/$p"
      cp -R "$pro_dir/$p/." "$root/$p/"
    fi
  done
  export CHITACH_PRO=1
elif [ -d "$root/src-tauri/src/pro" ]; then
  echo "Chitach Pro: already in this checkout"
  export CHITACH_PRO=1
else
  echo "Chitach Pro: private repo not found, building Chitach Free"
  export CHITACH_PRO=0
fi

if [ "$#" -gt 0 ]; then
  cd "$root"
  exec "$@"
fi
