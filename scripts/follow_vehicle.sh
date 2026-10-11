#!/usr/bin/env bash
# Follow any existing vehicle without changing its orders or the game.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
game="${1:-}"
vehicle="${2:-}"
binary="${OPENTTDRS_CLIENT_BIN:-$root/target/release/openttdrs-client}"
[[ "$game" = /* ]] || game="$PWD/$game"
[[ "$binary" = /* ]] || binary="$PWD/$binary"
if [[ $# -ne 2 || ! -f "$game" || ! -x "$binary" || ! "$vehicle" =~ ^[0-9]+$ ]]; then
    echo "Usage: $0 game.sav|game.json vehicle-id; build the release client first" >&2
    exit 1
fi
case "$game" in
    *.sav) load="OPENTTDRS_SAV_LOAD=$game" ;;
    *.json) load="OTTDJSON_LOAD=$game" ;;
    *) echo "Expected a .sav or .json game" >&2; exit 1 ;;
esac
cd -- "$root"
exec env -u OTTDMAP_FILE -u OTTDJSON_LOAD -u OPENTTDRS_SAV_LOAD \
    "$load" "OPENTTDRS_FOLLOW_VEHICLE=$vehicle" "$binary"
