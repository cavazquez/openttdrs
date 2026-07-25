# Documentación — openttdrs

Un archivo vivo por temática. Orden sugerido al entrar:

1. [ARCHITECTURE.md](ARCHITECTURE.md) · [adr/](adr/) — gobierno y capas
2. [PLANIFICACION.md](PLANIFICACION.md) — roadmaps, gaps, sprints, checklists
3. [PARIDAD.md](PARIDAD.md) — madurez, mapeos, oráculos
4. [MAPA_Y_FERROCARRIL.md](MAPA_Y_FERROCARRIL.md) — saves, tiles, señales, vías
5. [GRAFICOS.md](GRAFICOS.md) — OpenGFX y bugs visuales
6. [RENDIMIENTO.md](RENDIMIENTO.md) — mapas grandes y benches

**Issues de backlog:** [github.com/cavazquez/openttdrs/issues](https://github.com/cavazquez/openttdrs/issues).

---

| Documento | Uso |
|-----------|-----|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Capas core→Bevy→net, diseño I0–I8, inventarios, informe upstream |
| [adr/](adr/) | Decisiones inmutables (multiplayer, tick 37 Hz, determinismo, host migration) |
| [PLANIFICACION.md](PLANIFICACION.md) | Roadmaps P0–P3 / UI / sprints / industrias / SAV / drag; SP1; dev bot |
| [PARIDAD.md](PARIDAD.md) | Status road/rail, mappings, gaps UI, snapshots, PBS/Airport oracles |
| [MAPA_Y_FERROCARRIL.md](MAPA_Y_FERROCARRIL.md) | Flujo `.ottdmap`, MAPT/chunks, señales, autorail, waypoints |
| [GRAFICOS.md](GRAFICOS.md) | Sprites OpenGFX + handoff terreno |
| [RENDIMIENTO.md](RENDIMIENTO.md) | PERF mapas grandes + benchmarks |

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

*Última actualización: 2026-07-25 (consolidación temática)*
