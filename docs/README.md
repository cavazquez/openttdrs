# Documentación — openttdrs

Índice de `docs/`. Orden sugerido al entrar al proyecto:

1. [ROADMAP_AUDITORIA_2026.md](ROADMAP_AUDITORIA_2026.md) — roadmap maestro post-auditoría ([#121](https://github.com/cavazquez/openttdrs/issues/121))
2. [ROADMAP_PARIDAD_UI_GLOBAL.md](ROADMAP_PARIDAD_UI_GLOBAL.md) — paridad UI + NewGRF (fuente de “siguiente corte”)
3. [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) — sprints del hito 0.1
4. [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) — gaps vs OpenTTD
5. [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md) — hallazgos técnicos fijos y comandos
6. [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) — filosofía I0–I8
7. [adr/0001-multiplayer-v1.md](adr/0001-multiplayer-v1.md) — arquitectura red v1 (listen-server + dedicated)

**Issues de backlog:** [github.com/cavazquez/openttdrs/issues](https://github.com/cavazquez/openttdrs/issues) (abiertas desde los ROADMAP, jul 2026).

---

## Planificación y producto

| Documento | Uso |
|-----------|-----|
| [adr/0001-multiplayer-v1.md](adr/0001-multiplayer-v1.md) | ADR: lockstep, listen-server + dedicated headless, sin host migration v1 |
| Crate `openttdrs-net` | TCP I8; bin `openttdrs-dedicated`; cliente `--server` / `--client` |
| [INVENTARIO_HASHMAP_DETERMINISMO.md](INVENTARIO_HASHMAP_DETERMINISMO.md) | #115: HashMap/HashSet vs hash canónico |
| [INVENTARIO_MUTACIONES_CLIENTE.md](INVENTARIO_MUTACIONES_CLIENTE.md) | #114: mutaciones `SimWorld` fuera de `Command` |
| [ROADMAP_AUDITORIA_2026.md](ROADMAP_AUDITORIA_2026.md) | Tablero vivo del roadmap #121 (fases, camino crítico, DoD) |
| [ROADMAP_PARIDAD_UI_GLOBAL.md](ROADMAP_PARIDAD_UI_GLOBAL.md) | UI-0…UI-8, NewGRF Action0–14, siguiente corte |
| [ROADMAP_PARIDAD_ESTRUCTURAL.md](ROADMAP_PARIDAD_ESTRUCTURAL.md) | Cargo packets, YAPF/PBS, economía, mono/maglev |
| [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) | Sprints S1–S6, criterios de cierre 0.1 |
| [ROADMAP_IMPORTACION_OPENTTD.md](ROADMAP_IMPORTACION_OPENTTD.md) | Animaciones, audio, dinámicas importables |
| [ROADMAP_MENUS_UI.md](ROADMAP_MENUS_UI.md) | Menús de flota (órdenes, vehículo, depósito) |
| [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) | Inventario features vs original |
| [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) | Spec I0–I8 |
| [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md) | Hallazgos de tiles/sprites, comandos |
| [ROADMAP_PARIDAD_VISUAL.md](ROADMAP_PARIDAD_VISUAL.md) | Checklist visual SP3 |
| [ROADMAP_TERRAFORM.md](ROADMAP_TERRAFORM.md) | Elevar / bajar / nivelar / gen mundo |
| [ROADMAP_NEWS_STATUSBAR.md](ROADMAP_NEWS_STATUSBAR.md) | Barra inferior y noticias |
| [ROADMAP_MAIN_MENU.md](ROADMAP_MAIN_MENU.md) | Menú inicio / Nueva partida |
| [ROADMAP_CARRETERAS_DRAG.md](ROADMAP_CARRETERAS_DRAG.md) | Handoff drag carretera |
| [ROADMAP_JUNCTIONARY.md](ROADMAP_JUNCTIONARY.md) | Cruces ferroviarios (Junctionary) |
| [ROADMAP_SAV_EXPORT.md](ROADMAP_SAV_EXPORT.md) | Export `.sav` |
| [ROADMAP_INDUSTRIAS_PARIDAD.md](ROADMAP_INDUSTRIAS_PARIDAD.md) | Industrias gfx 0–174 / NewGRF ≥175 |
| [DEV_BOT.md](DEV_BOT.md) | Sonda headless carga/ingresos |
| [epics/ai_rivals.md](epics/ai_rivals.md) | Épica IA rivales |
| [parity/RAIL_REVIEW_HANDOFF.md](parity/RAIL_REVIEW_HANDOFF.md) | Handoff revisión ferroviaria |

### Organización del código

| Área | Ruta |
|------|------|
| Menú inicio | `crates/openttdrs-client/src/ui/main_menu/` |
| Flota | `ui/vehicle_window.rs`, `ui/toolbar/order_panel/`, `depot_panel.rs` |
| NewGRF render | `render/station_newgrf.rs`, `road_newgrf.rs`, `vehicles.rs` |
| Población procedural | `state/bootstrap/procedural_population/` |
| Comandos transporte | `openttdrs-core/src/command/transport/` |
| Tests rail por dominio | `openttdrs-core/src/command/tests/rail/` (#139) |
| Action2 / sprites GRF | `openttdrs-core/src/newgrf_sprites.rs`, `station_action2.rs`, `road_action2.rs` |

---

## Mapa, saves y flujo

| Documento | Uso |
|-----------|-----|
| [FLUJO_MAPA_Y_CLIENTE.md](FLUJO_MAPA_Y_CLIENTE.md) | Save → `.ottdmap` → cliente → JSON |
| [OTTDMAP_FORMAT.md](OTTDMAP_FORMAT.md) | Spec binaria `.ottdmap` |
| [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) | MAPT, `m5`, chunks |
| [SENALES_FERROVIARIAS.md](SENALES_FERROVIARIAS.md) | Señales, PBS, pick diagonal |
| [VIAS_FERROVIARIAS_COLOCACION.md](VIAS_FERROVIARIAS_COLOCACION.md) | Autorail vs bits, uniones |
| [SNAPSHOT_ORACLE_WORKFLOW.md](SNAPSHOT_ORACLE_WORKFLOW.md) | Comparación con fork oráculo |

---

## Gráficos y render

| Documento | Uso |
|-----------|-----|
| [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md) | Extracción, isometría, locomotoras |
| [SPRITES_OPENGFX_COMPLETO.md](SPRITES_OPENGFX_COMPLETO.md) | Catálogo de IDs |
| [INDUSTRIAS_OPENGFX.md](INDUSTRIAS_OPENGFX.md) | gfx → sprite |
| [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md) | Auditoría PNG / fixtures |
| [HANDOFF_BUGS_VISUALES_TERRAIN.md](HANDOFF_BUGS_VISUALES_TERRAIN.md) | Bugs visuales terreno |
| [HANDOFF_WAYPOINTS_RAIL.md](HANDOFF_WAYPOINTS_RAIL.md) | Waypoints rail |

---

## Construcción (SP2 cerrado)

| Documento | Uso |
|-----------|-----|
| [SP2_CHECKLIST.md](SP2_CHECKLIST.md) | Checklist construcción |
| [SP1_CHECKLIST.md](SP1_CHECKLIST.md) | Ciclo jugable (sesión manual) |
| [SP2_PARADAS_Y_ESTACIONES.md](SP2_PARADAS_Y_ESTACIONES.md) | Paradas bus/camión/tren |

---

## Referencia upstream

| Documento | Uso |
|-----------|-----|
| [parity/openttd-reference.json](parity/openttd-reference.json) | Manifiesto pin (#109): URL, commit, tag, licencia |
| [parity/OPENTTD_REFERENCE.md](parity/OPENTTD_REFERENCE.md) | Cómo clonar / actualizar la referencia |
| [parity/SNAPSHOT_SCHEMA.md](parity/SNAPSHOT_SCHEMA.md) | Esquema JSON oráculo ↔ candidato (#110) |
| [parity/SNAPSHOT_FIRST_DIVERGENCE.md](parity/SNAPSHOT_FIRST_DIVERGENCE.md) | Evidencia: road_bits_hash diverge en stationlist |
| [SNAPSHOT_ORACLE_WORKFLOW.md](SNAPSHOT_ORACLE_WORKFLOW.md) | Flujo oráculo OpenTTD C++ vs snapshot_dumper |
| [INFORME_ARQUITECTURA_OPENTTD.md](INFORME_ARQUITECTURA_OPENTTD.md) | Arquitectura C++ (clon local) |

Clon: `./scripts/fetch-openttd-reference.sh` → `reference/openttd-upstream/` (commit del manifiesto).

---

## Archivo histórico

Planes cerrados o absorbidos — no son fuente de verdad actual: [archive/README.md](archive/README.md).

---

*Última actualización: 2026-07-12 (NewGRF Action2, issues de backlog, índice alineado al README raíz)*
