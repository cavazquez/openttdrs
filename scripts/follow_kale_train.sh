#!/usr/bin/env bash
# Follow an existing train using the same interpolated position as its sprite.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
game="${1:-$root/save/Kale_TitleGame.sav}"
train="${2:-2902}"
binary="${OPENTTDRS_CLIENT_BIN:-$root/target/release/openttdrs-client}"
[[ "$game" = /* ]] || game="$PWD/$game"
[[ "$binary" = /* ]] || binary="$PWD/$binary"
if [[ ! -f "$game" || ! -x "$binary" || ! "$train" =~ ^[0-9]+$ ]]; then
    echo "Usage: $0 [game.sav|game.json] [train id]; build the release client first" >&2
    exit 1
fi
case "$game" in
    *.sav) load="OPENTTDRS_SAV_LOAD=$game" ;;
    *.json) load="OTTDJSON_LOAD=$game" ;;
    *) echo "Expected a .sav or .json game" >&2; exit 1 ;;
esac
cd -- "$root"
exec env -u OTTDMAP_FILE -u OTTDJSON_LOAD -u OPENTTDRS_SAV_LOAD \
    "$load" "OPENTTDRS_FOLLOW_VEHICLE=$train" "$binary"
