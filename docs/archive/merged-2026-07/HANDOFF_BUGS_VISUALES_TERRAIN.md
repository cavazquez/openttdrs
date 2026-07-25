# Handoff: bugs visuales de terreno (vivo)

**Audiencia:** agentes / mantenedores.  
**Detalle histórico** (intentos, diffs, hipótesis largas):
[archive/HANDOFF_BUGS_VISUALES_TERRAIN.md](archive/HANDOFF_BUGS_VISUALES_TERRAIN.md).

Waypoints rail (cerrado): [HANDOFF_WAYPOINTS_RAIL.md](HANDOFF_WAYPOINTS_RAIL.md).

---

## Abiertos / a verificar

| # | Síntoma | Estado |
|---|---------|--------|
| C | Rectángulo verde semitransparente al iniciar (ghost construcción) | Abierto |
| D | Casas Toyland en pueblo templado | Abierto |
| E | Orillas de agua con artefactos blancos | Abierto (menor) |
| A | Tablero marrón/verde al iniciar | Parcialmente mitigado |
| B | Rombo de teselas oscuras al construir carretera | Parcialmente mitigado |

## Reproducción rápida

```bash
cd openttdrs
OPENTTDRS_JSON_SAVE=save/partida_2026-06-22_0942.json cargo run -p openttdrs-client
```

Ghost (C): entrar con herramienta paisaje (p. ej. Plantar bosque) activa; el
preview sigue al cursor desde el primer frame.

## Pistas de código

- Ghost: `update_build_ghost_preview` / toolbar landscape.
- Remap / culling: `render/world.rs`, `ui/toolbar/build_input/click.rs`.
- Casas / clima: `HOUSE_DRAW_DATA`, gen procedural; ver archive § D.
- Bytes tesela: [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md).

Al cerrar un bug: marcar aquí y, si aporta fórmulas, una línea en TILES o
`SPRITES_OPENGFX.md` — no reexpandir este handoff.
