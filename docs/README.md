# Documentación — openttdrs

Un archivo vivo por temática. Orden sugerido al entrar:

1. [ARCHITECTURE.md](ARCHITECTURE.md) · [adr/](adr/) — gobierno y capas
2. [PLANIFICACION.md](PLANIFICACION.md) — roadmaps, gaps, sprints, checklists
3. [PARIDAD.md](PARIDAD.md) — madurez, mapeos, oráculos
4. [MAPA_Y_FERROCARRIL.md](MAPA_Y_FERROCARRIL.md) — saves, tiles, señales, vías
5. [GRAFICOS.md](GRAFICOS.md) — OpenGFX y bugs visuales
6. [RENDIMIENTO.md](RENDIMIENTO.md) — mapas grandes y benches
7. [parity/METODOLOGIA_RENDER_SAV.md](parity/METODOLOGIA_RENDER_SAV.md) — método reproducible para investigar render de `.sav`
8. [TELEGRAM_CI.md](TELEGRAM_CI.md) — notificación opcional de commits por Telegram

**Issues de backlog:** [github.com/cavazquez/openttdrs/issues](https://github.com/cavazquez/openttdrs/issues).

## Propiedad del estado (no duplicar trabajo)

| Tema | Fuente canónica | Documentos de apoyo |
|---|---|---|
| Madurez global, road y rail | [PARIDAD.md](PARIDAD.md) | Roadmaps y guía de mapa sólo enlazan este estado |
| Compatibilidad `.sav` import/export | [parity/sav-compatibility.md](parity/sav-compatibility.md) | [PLANIFICACION.md](PLANIFICACION.md#export-sav) explica el writer; [MAPA_Y_FERROCARRIL.md](MAPA_Y_FERROCARRIL.md) el formato/pipeline |
| NewGRF Action0/3/5 | [parity/newgrf-action0-matrix.md](parity/newgrf-action0-matrix.md) | Propiedades de catálogos y render |
| NewGRF callbacks | [parity/newgrf-callback-matrix.md](parity/newgrf-callback-matrix.md) | Call sites, storage y residual |

Al cambiar una capacidad, editar su fuente canónica y el resumen de
`PARIDAD.md` sólo si modifica la madurez global. Los roadmaps, README y guías
técnicas no deben duplicar listas de soporte ni usar inventarios históricos como
backlog.

---

| Documento | Uso |
|-----------|-----|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Capas core→Bevy→net, diseño I0–I8, inventarios, informe upstream |
| [adr/](adr/) | Decisiones inmutables (multiplayer, tick 37 Hz, determinismo, host migration) |
| [PLANIFICACION.md](PLANIFICACION.md) | Roadmaps P0–P3 / UI / sprints / industrias / SAV / drag; SP1; dev bot |
| [PARIDAD.md](PARIDAD.md) | Status road/rail, mappings, gaps UI, snapshots, PBS/Airport oracles |
| [parity/sav-compatibility.md](parity/sav-compatibility.md) | Matriz única de importación/exportación `.sav` |
| [parity/newgrf-action0-matrix.md](parity/newgrf-action0-matrix.md) | Propiedades y runtime Action0/3/5 |
| [parity/newgrf-callback-matrix.md](parity/newgrf-callback-matrix.md) | Ejecución real de callbacks NewGRF |
| [MAPA_Y_FERROCARRIL.md](MAPA_Y_FERROCARRIL.md) | Flujo `.ottdmap`, MAPT/chunks, señales, autorail, waypoints |
| [GRAFICOS.md](GRAFICOS.md) | Sprites OpenGFX + handoff terreno |
| [RENDIMIENTO.md](RENDIMIENTO.md) | PERF mapas grandes + benchmarks |
| [parity/METODOLOGIA_RENDER_SAV.md](parity/METODOLOGIA_RENDER_SAV.md) | Capas raw/semántica/draw, oráculo C++ y flujo de depuración de partidas `.sav` |
| [REFERENCIAS_EXTERNAS.md](REFERENCIAS_EXTERNAS.md) | Proyectos locales de consulta (OpenLoco / FreeRCT) |
| [TELEGRAM_CI.md](TELEGRAM_CI.md) | Secrets y comportamiento de avisos por Telegram en GitHub Actions |

### Assets (no markdown)

| Ruta | Uso |
|------|-----|
| [parity/openttd-reference.json](parity/openttd-reference.json) | Pin commit OpenTTD |
| [parity/screenshots/](parity/screenshots/) | Capturas de paridad |
| `parity/*.html` | Trazas (p. ej. PBS) |

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

## Archivo histórico

Planes y roadmaps cerrados o absorbidos en esta consolidación:

- [archive/README.md](archive/README.md)
- [archive/merged-2026-07/](archive/merged-2026-07/) — fuentes previas a un-archivo-por-categoría

---

*Última actualización: 2026-08-14 (propiedad explícita de estado SAV/NewGRF/road/rail)*
