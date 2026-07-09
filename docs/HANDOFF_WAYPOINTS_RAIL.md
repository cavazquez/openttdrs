# Handoff: waypoints ferroviarios

**Estado (jul 2026):** vía de fondo 1011/1012 + 4 capas ogfx2 (cuerpo + toldos CC).
**TILE_SEQ dx/dy = 0** — la separación oeste/este va solo en xrel NFO (−30/−8).

## Por qué no dy=13

El prop 1A de `ogfx2_stations` declara parents en `(0,13)` / `(13,0)`, pero OpenTTD
resuelve Action1 con var10/registros. En openttdrs los PNG exportados ya incluyen
el xrel de separación; sumar dy=13 duplica el offset → **dos casetas**, una en la
hierba (confirmado en captura del usuario, jul 2026).

## Regenerar

```bash
python3 scripts/gen_rail_waypoint_sprites.py
python3 scripts/gen_rail_station_draw_data.py
python3 scripts/gen_tile_atlas.py
cargo test -p openttdrs-client rail_waypoint
```

## Archivos

| Archivo | Rol |
|---------|-----|
| `sprites/station.rs` | `RAIL_WAYPOINT_SEQ_*` (dx=dy=0) |
| `render/tiles/objects.rs` | ground + overlays |
| `ui/toolbar/preview/rail_waypoint.rs` | Fantasma |
| `scripts/gen_rail_waypoint_sprites.py` | PNG #19–26 |
