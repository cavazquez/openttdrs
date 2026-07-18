#!/usr/bin/env python3
"""Extrae sprites de OVNI (desastres) desde OpenGFX.

OpenTTD `table/sprites.h`:
  - SPR_UFO_SMALL_SCOUT  = 3908
  - SPR_UFO_HARVESTER    = 3920
  - variante oscura scout = 3909 (sombra)

Uso: python3 scripts/gen_ufo_sprites.py
"""
from __future__ import annotations

from gen_field_draw_data import REPO, Cropper
from nfo_sprite_meta import detect_graphics_mode

# (sprite_id, png_name)
UFO_SPRITES: list[tuple[int, str]] = [
    (3908, "ufo_small_scout.png"),
    (3909, "ufo_small_scout_darker.png"),
    (3920, "ufo_harvester.png"),
]


def main() -> None:
    mode = detect_graphics_mode(REPO) or "8bpp"
    cropper = Cropper(mode)
    for sid, name in UFO_SPRITES:
        cropper.crop(sid, name)
        print(f"Recortado sprite {sid} → {name}")


if __name__ == "__main__":
    main()
