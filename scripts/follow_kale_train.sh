#!/usr/bin/env bash
# Compatibility launcher for Kale's train; the follower supports every kind.
set -euo pipefail
root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
game="${1:-$root/save/Kale_TitleGame.sav}"
train="${2:-2902}"
exec bash "$root/scripts/follow_vehicle.sh" "$game" "$train"
