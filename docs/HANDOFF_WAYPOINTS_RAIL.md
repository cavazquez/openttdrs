# Handoff: waypoints ferroviarios

**Estado (jul 2026):** vía de fondo 1011/1012 + 4 capas ogfx2 (cuerpo + toldos CC).

**Posicionamiento correcto:** TILE_SEQ `dy=13` (eje X) / `dx=13` (eje Y) **y**
mitades este con el **mismo xrel/yrel** que el ancla oeste
(`rail_waypoint_layer_meta`). El tamaño `w`/`h` sigue siendo el del PNG este.

## Por qué no xrel NFO este (−8) + dy=13

Sumar el xrel NFO de la mitad este **y** TILE_SEQ dy=13 duplica el offset →
una caseta en la vía y otra en la hierba.

## Por qué no dy=0 + xrel distintos (−30/−8)

Sin TILE_SEQ las mitades quedan en la misma fila de pantalla pero con anclas
independientes → forma de **V** sobre la vía (capturas jul 2026).

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
| `sprites/station.rs` | `RAIL_WAYPOINT_SEQ_*` + `rail_waypoint_layer_meta` |
| `render/tiles/objects.rs` | ground + overlays |
| `ui/toolbar/preview/rail_waypoint.rs` | Fantasma |
| `scripts/gen_rail_waypoint_sprites.py` | PNG #19–26 |
