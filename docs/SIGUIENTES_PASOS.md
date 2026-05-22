# Siguientes pasos — openttdrs

Documento vivo: **qué documentar**, **dónde está**, y **cómo seguir** el desarrollo con
prioridad en **juego en solitario** (hito 0.1). La fundación **I0–I7** ya está en `main`;
**I8 (red / multijugador)** queda en backlog de mínima prioridad.

---

## Índice de documentación técnica

| Documento | Contenido |
|-----------|-----------|
| [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) | Filosofía por incrementos, spec I0–I8, **estado actual** del repo. |
| [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md) | NFO, transparencia 8bpp, IDs de sprite, proyección isométrica, tabla road_tx/ty. |
| [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) | MAPT, `m5` (carretera normal / cruce / depósito), `.ottdmap`, relieve 8px, referencias upstream. |
| [INFORME_ARQUITECTURA_OPENTTD.md](INFORME_ARQUITECTURA_OPENTTD.md) | Visión del código de referencia en `reference/openttd-upstream/`. |

---

## Hallazgos ya fijados en código y docs

1. **Sprite “coal mine” equivocado** — el ID antiguo era de otra industria; headframe correcto
   y proceso de verificación: `SPRITES_OPENGFX.md` + commits históricos.
2. **Cruces a nivel** — no usar bits 0–3 de `m5` como road bits; el eje de la carretera va
   en el bit 0. Detalle en `TILES_Y_SAVEGAMES_OPENTTD.md`.
3. **MAPT + `m5`** — hace falta el byte MAPT crudo en `Tile` para decodificar túneles/puentes
   frente a `MP_ROAD`.
4. **Intercambio `road_tx` ↔ `road_ty`** — respecto a `RoadDir`, para alinear textura OpenGFX
   con la isometría del cliente; **validado visualmente** en mapas `.ottdmap`.
5. **Limitaciones actuales (visual)** — carretera **plana** usa `road_flat_00..18` (`GetRoadSpriteOffset`);
   faltan sobre todo **pendientes**, **estaciones de tren** completas y calibración fina de industrias;
   ver [PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md).
6. **Industrias sandbox** — `PlaceIndustrySpec` usa layouts base de OpenTTD para mina de carbón,
   fábrica, granja, bosque, refinería, pozos, aserradero y minas. El ghost de construcción consume
   la misma plantilla (`openttdrs_core::industry_template`), así que el footprint previsto y el
   construido comparten una única fuente de verdad. Refinería quedó visualmente muy cercana al
   original; `Farm`, `Factory` y `Coal Mine` son aceptables, pero podrían mejorar con calibración
   fina de offsets/capas.
7. **Fuente UI UTF-8** — `static/fonts/DejaVuSansMono.ttf` es un asset versionado separado de
   `assets/`; se usa para que Bevy renderice acentos en paneles (`Fábrica`, `Refinería`,
   `Petróleo`). `assets/` queda reservado para gráficos/sonidos generados por scripts e ignorados.

## Estado del refactor del cliente

- `main.rs` conserva el armado de la app y los sistemas principales, pero el render de mapa vive
  en `crates/openttdrs-client/src/render/`.
- El render/índice de vehículos quedó aislado en `vehicle_render.rs`, separando esa lógica de la
  construcción de teselas.
- Las variables de entorno del cliente se leen desde `config.rs` para evitar parsing duplicado.
- `RenderGrid` tiene tests para la inferencia de costa en agua exportada con `m5=0`.

---

## Cómo seguir (prioridades)

**Orden de producto:** cerrar **0.1 en solitario** (fases SP) antes de invertir en **I8 (red)**.
Dentro de SP, visual (SP3) y gameplay (SP1–SP2) pueden avanzar en paralelo según lo que
más moleste al jugar.

### SP1 — Ciclo jugable (prioridad alta)

- Economía y estadísticas **legibles** en HUD (dinero, cargas, vehículos sin ruta).
- Órdenes y estaciones: flujo claro desde toolbar → mapa → simulación.
- Coherencia **estación en mapa** vs entradas en `state.stations` (el bootstrap demo aún puede
  reservar posiciones en hierba sin `TileKind::Station`).
- Tests de integración UI↔`apply_command` ampliados donde falten herramientas críticas.

### SP2 — Construcción y herramientas (prioridad alta)

- Mensajes de error de colocación (`CommandError` → feedback HUD / toolbar).
- Preview/validación para industria, estación, túnel, puente, depósito.
- Paneles de órdenes, depósito y carga de estación estables en mapas reales y procedurales.

### SP3 — Presentación del mapa (prioridad media)

Plan detallado (revisión upstream + estado del cliente): **[PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md)**.

**Auditoría SP3.0:** `python3 scripts/audit_sp3_assets.py` — resultado en [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md).

Resumen de huecos reales (mucho de lo “esquina/T + trackbits” **ya está** en `road_flat_*` / `collect_rail_sprites`):

- **Pendientes** carretera/vía en teselas inclinadas (familias slope upstream).
- **Estaciones** de tren: plataformas/edificios, no solo suelo bus/camión.
- **Casas/industrias** en `.ottdmap`: ampliar tablas gfx y quitar fallbacks genéricos.
- **Assets**: auditar `rail_*.png` (evitar placeholders del script).
- **Rendimiento**: culling al dibujar el mapa (el agua ya culling; el resto del mapa no).
- **Agua/costa**: validar Coast en saves reales; animación mar ya aproximada.

Clon de referencia C++: `bash scripts/fetch-openttd-reference.sh`.

### SP4 — Pulido y deuda (prioridad media)

- ~~Alinear `./scripts/check.sh ci` con CI~~ — hecho: `fmt-check`, clippy `-D warnings`, nextest/test, TNBP, golden, `py_compile`.
- Migraciones de save si el esquema JSON cambia.
- Mantener docs y tests al día con el refactor modular del cliente/core.

### Fundación incremental (referencia — hecho en `main`)

- **I0–I5** — mapa, industria, vehículos, cargo, pathfinding.
- **I6** — comandos del jugador (`openttdrs_core::command`, toolbar en cliente).
- **I7** — save/load JSON (`save/`, F5/F9, `OPENTTDRS_JSON_SAVE`).

### I8 — Red / multijugador (mínima prioridad, post-0.1)

- Log de comandos, `apply_command_log`, transporte TCP, `--server` / `--client`.
- Spec en [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) § Incremento 8; **no** bloquea el cierre del 0.1.

### Higiene y referencia

- Mantener `reference/openttd-upstream/` actualizado (`scripts/fetch-openttd-reference.sh`).
- Tests que carguen un `.ottdmap` pequeño en memoria y comprueben `effective_road_bits` /
  dimensiones (opcional, sin Bevy).
- Seguir achicando `main.rs`: buenos candidatos son hotkeys/save-load, debug gizmos y animación
  de agua.

---

## Comandos útiles

```bash
# Mapa real
python3 scripts/parse_sav.py partida.sav assets/maps/mapa.ottdmap
OTTDMAP_FILE=assets/maps/mapa.ottdmap cargo run -p openttdrs-client

# Solo demo procedural
cargo run -p openttdrs-client

# Tests core
cargo test -p openttdrs-core

# Validar panel/ghost de industrias
cargo test -p openttdrs-client industry
cargo test -p openttdrs-client preview
```

---

## Si algo se pierde otra vez

1. Buscar en los cuatro MD de `docs/` anteriores.
2. Mirar comentarios en `crates/openttdrs-client/src/main.rs` (`effective_road_bits`, sprites).
3. Upstream: `road_map.h`, `road_func.h`, `tile_map.h`, `saveload/map_sl.cpp`.
