# Handoff: waypoints ferroviarios

**Estado (jul 2026):** layout ogfx2 Action 2 parcial cerrado en código — vía de fondo
1011/1012 + 4 capas (cuerpo 4974–4977 + toldos CC 4978–4981). Verificar visualmente
en partida; si sigue mal, ver § Hipótesis restantes.

---

## Comportamiento actual

1. Ground: `rail_station_ground_track_sprite` (1012 eje X / 1011 eje Y).
2. Capas: `rail_waypoint_draw_layers` — 4 sprites, TILE_SEQ dx/dy/dz = 0.
3. Assets: `scripts/gen_rail_waypoint_sprites.py` exporta ogfx2 #19–26.
4. Preview toolbar: misma vía + capas.

## Regenerar

```bash
python3 scripts/gen_rail_waypoint_sprites.py
python3 scripts/gen_rail_station_draw_data.py
python3 scripts/gen_tile_atlas.py
cargo test -p openttdrs-client rail_waypoint
```

## Hipótesis restantes (si el usuario sigue viendo mal)

- Atlas/binario desactualizado → `cargo clean` + regenerar + `cargo run`.
- Layout Action 2 con offsets distintos a xrel NFO (decodificar NFO líneas 32–33).
- Ancla Bevy vs OpenTTD en toldos 21/22.
- Eje `m5` invertido en saves importados.

## Archivos clave

| Archivo | Rol |
|---------|-----|
| `sprites/station.rs` | `RAIL_WAYPOINT_SEQ_*` |
| `render/tiles/objects.rs` | `spawn_station_tile` |
| `ui/toolbar/preview/rail_waypoint.rs` | Fantasma |
| `scripts/gen_rail_waypoint_sprites.py` | PNG ogfx2 |
| `scripts/gen_rail_station_draw_data.py` | Metadata NFO |
