# Documentación — openttdrs

Índice de `docs/`. Orden sugerido al entrar al proyecto:

0. [../CONTRIBUTING.md](../CONTRIBUTING.md) · [ARCHITECTURE.md](ARCHITECTURE.md) · [adr/](adr/) — gobierno
1. [ROADMAP_PARIDAD_UI_GLOBAL.md](ROADMAP_PARIDAD_UI_GLOBAL.md) — paridad UI + NewGRF (fuente de “siguiente corte”)
2. [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) — sprints del hito 0.1
3. [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) — vista corta de gaps vs OpenTTD
4. [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md) — hallazgos técnicos fijos y comandos
5. [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) — filosofía I0–I8
6. [adr/0001-multiplayer-v1.md](adr/0001-multiplayer-v1.md) — arquitectura red v1
7. [adr/0003-tick-37hz-openttd.md](adr/0003-tick-37hz-openttd.md) — tick ~37 Hz; [0002](adr/0002-determinismo-tick-referencia.md) hash/pin

**Issues de backlog:** [github.com/cavazquez/openttdrs/issues](https://github.com/cavazquez/openttdrs/issues).

---

## Planificación y producto

| Documento | Uso |
|-----------|-----|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Capas core → Bevy → net |
| [adr/README.md](adr/README.md) | Índice + plantilla de ADRs |
| [adr/0001-multiplayer-v1.md](adr/0001-multiplayer-v1.md) | ADR: lockstep, listen-server + dedicated |
| [adr/0003-tick-37hz-openttd.md](adr/0003-tick-37hz-openttd.md) | ADR: tick ~37 Hz |
| [adr/0002-determinismo-tick-referencia.md](adr/0002-determinismo-tick-referencia.md) | ADR: determinismo + pin OpenTTD |
| [adr/0004-host-migration-post-v1.md](adr/0004-host-migration-post-v1.md) | ADR: host migration |
| Crate `openttdrs-net` | TCP I8; bin `openttdrs-dedicated`; `--server` / `--client` |
| [INVENTARIO_HASHMAP_DETERMINISMO.md](INVENTARIO_HASHMAP_DETERMINISMO.md) | #115 |
| [INVENTARIO_MUTACIONES_CLIENTE.md](INVENTARIO_MUTACIONES_CLIENTE.md) | #114 |
| [ROADMAP_PARIDAD_UI_GLOBAL.md](ROADMAP_PARIDAD_UI_GLOBAL.md) | Siguiente corte UI (detalle en archive) |
| [ROADMAP_PARIDAD_ESTRUCTURAL.md](ROADMAP_PARIDAD_ESTRUCTURAL.md) | Consist, PBS, economía, mono/maglev |
| [ROADMAP_PARIDAD_SIMULACION.md](ROADMAP_PARIDAD_SIMULACION.md) | Paridad de comportamiento P0–P3 (auditoría 15.3) |
| [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) | Sprints S1–S6 |
| [ROADMAP_IMPORTACION_OPENTTD.md](ROADMAP_IMPORTACION_OPENTTD.md) | Animaciones, audio, dinámicas |
| [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) | Vista corta de gaps |
| [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) | Spec I0–I8 |
| [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md) | Hallazgos y comandos |
| [ROADMAP_CARRETERAS_DRAG.md](ROADMAP_CARRETERAS_DRAG.md) | Handoff drag carretera (paused) |
| [ROADMAP_JUNCTIONARY.md](ROADMAP_JUNCTIONARY.md) | Cruces ferroviarios |
| [ROADMAP_SAV_EXPORT.md](ROADMAP_SAV_EXPORT.md) | Export `.sav` |
| [ROADMAP_INDUSTRIAS_PARIDAD.md](ROADMAP_INDUSTRIAS_PARIDAD.md) | Industrias gfx / NewGRF |
| [DEV_BOT.md](DEV_BOT.md) | Sonda headless carga/ingresos |
| [archive/epics/ai_build_pathfind.md](archive/epics/ai_build_pathfind.md) | Épica #184 pathfind IA (cerrada) |
| Rivales IA | TransCargo (rail) + RoadHaul (buses); toggle «Rival IA» en nueva partida |
| [parity/status.md](parity/status.md) · [parity/rail_status.md](parity/rail_status.md) | Madurez road/rail |
| [parity/MAPPING.md](parity/MAPPING.md) | Índice mapeos C++ ↔ Rust |
| [parity/ui_windows_parity.md](parity/ui_windows_parity.md) | Paridad ventanas UI |

### Organización del código

| Área | Ruta |
|------|------|
| Menú inicio | `crates/openttdrs-client/src/ui/main_menu/` |
| Flota | `ui/vehicle_window.rs`, `ui/toolbar/order_panel/`, `depot_panel.rs` |
| NewGRF render | `render/station_newgrf.rs`, `road_newgrf.rs`, `vehicles.rs` |
| Población procedural | `state/bootstrap/procedural_population/` |
| Comandos transporte | `openttdrs-core/src/command/transport/` |
| Tests rail por dominio | `openttdrs-core/src/command/tests/rail/` |
| Action2 / sprites GRF | `openttdrs-core/src/newgrf_sprites.rs`, `station_action2.rs`, `road_action2.rs` |

---

## Mapa, saves y flujo

| Documento | Uso |
|-----------|-----|
| [FLUJO_MAPA_Y_CLIENTE.md](FLUJO_MAPA_Y_CLIENTE.md) | Save → `.ottdmap` → cliente → JSON |
| [OTTDMAP_FORMAT.md](OTTDMAP_FORMAT.md) | Spec binaria `.ottdmap` |
| [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) | MAPT, `m5`, chunks |
| [SENALES_FERROVIARIAS.md](SENALES_FERROVIARIAS.md) | Señales, PBS, pick diagonal |
| [PBS_EXTERNAL_ORACLE.md](PBS_EXTERNAL_ORACLE.md) | Golden PBS externo OpenTTD ↔ openttdrs |
| [VIAS_FERROVIARIAS_COLOCACION.md](VIAS_FERROVIARIAS_COLOCACION.md) | Autorail vs bits, uniones |
| [SNAPSHOT_ORACLE_WORKFLOW.md](SNAPSHOT_ORACLE_WORKFLOW.md) | Comparación con fork oráculo |

---

## Gráficos y render

| Documento | Uso |
|-----------|-----|
| [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md) | Extracción, isometría, locomotoras (+ anexos de catálogo) |
| [HANDOFF_BUGS_VISUALES_TERRAIN.md](HANDOFF_BUGS_VISUALES_TERRAIN.md) | Bugs visuales terreno |
| [HANDOFF_WAYPOINTS_RAIL.md](HANDOFF_WAYPOINTS_RAIL.md) | Waypoints rail |

---

## Checklists vivos

| Documento | Uso |
|-----------|-----|
| [SP1_CHECKLIST.md](SP1_CHECKLIST.md) | Ciclo jugable (sesión manual) |

---

## Referencia upstream

| Documento | Uso |
|-----------|-----|
| [parity/openttd-reference.json](parity/openttd-reference.json) | Manifiesto pin (#109) |
| [parity/OPENTTD_REFERENCE.md](parity/OPENTTD_REFERENCE.md) | Cómo clonar / actualizar la referencia |
| [parity/SNAPSHOT_SCHEMA.md](parity/SNAPSHOT_SCHEMA.md) | Esquema JSON oráculo (#110) |
| [parity/SNAPSHOT_FIRST_DIVERGENCE.md](parity/SNAPSHOT_FIRST_DIVERGENCE.md) | Evidencia oráculo |
| [parity/GENERATED_TABLES.md](parity/GENERATED_TABLES.md) | Reproducibilidad `#119` |
| [SNAPSHOT_ORACLE_WORKFLOW.md](SNAPSHOT_ORACLE_WORKFLOW.md) | Flujo oráculo |
| [INFORME_ARQUITECTURA_OPENTTD.md](INFORME_ARQUITECTURA_OPENTTD.md) | Arquitectura C++ (clon local) |

Clon: `./scripts/fetch-openttd-reference.sh` → `reference/openttd-upstream/`.

---

## Archivo histórico

Planes y roadmaps cerrados: [archive/README.md](archive/README.md)  
(incluye auditoría #121, NEWS, terraform, SP2/SP3, menús UI, épicas IA/GS).

---

*Última actualización: 2026-07-17 (limpieza docs / roadmaps)*
