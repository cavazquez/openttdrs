# Handoff: waypoints ferroviarios

**Estado (jul 2026):** layout alineado con **ogfx2_stations Action0 prop 1A**
(sprite 32 del NFO): ground 1011/1012 + parents en `(0,0)` / `(0,13)` (eje X) o
`(13,0)` (eje Y), más toldos CC 21–26 en el mismo origen que cada mitad.

---

## Referencia (decodificada)

```
Layout X: ground=1012
  parent Action1+0 @ (0, 0, 0) extent 16×3×16  → rail_4974 / toldo 4978
  parent Action1+1 @ (0, 13, 0) extent 16×3×16 → rail_4975 / toldo 4979
Layout Y: ground=1011
  parent @ (0, 0, 0)  → 4976 / 4980
  parent @ (13, 0, 0) → 4977 / 4981
```

Vanilla `station_land.h` usaba `dy=11`/`dx=11`; ogfx2 usa **13** (igual que road waypoints).

## Regenerar assets

```bash
python3 scripts/gen_rail_waypoint_sprites.py
python3 scripts/gen_rail_station_draw_data.py
python3 scripts/gen_tile_atlas.py
cargo test -p openttdrs-client rail_waypoint
```

## Archivos

| Archivo | Rol |
|---------|-----|
| `sprites/station.rs` | `RAIL_WAYPOINT_SEQ_*` |
| `render/tiles/objects.rs` | `spawn_station_tile` |
| `ui/toolbar/preview/rail_waypoint.rs` | Fantasma |
| `scripts/gen_rail_waypoint_sprites.py` | PNG ogfx2 #19–26 |
