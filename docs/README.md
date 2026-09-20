# Documentación — openttdrs

Un archivo vivo por temática. **Dirección actual (2026-09-19):** el corte de
validación de la alpha está completado después de las entregas #568–#577. El
escenario guiado usado entonces fue retirado del cliente; sus fixtures técnicos
permanecen internos. [Auditoría y decisiones de backlog](audits/2026-09-18-direction.md).
Orden sugerido al entrar:

1. [ARCHITECTURE.md](ARCHITECTURE.md) · [adr/](adr/) — gobierno y capas
2. [Plan vigente](parity/continuous-work-plan.md) — entrega, orden y aceptación; [PLANIFICACION.md](PLANIFICACION.md) conserva guías históricas
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
| Dirección y backlog ejecutable | [parity/continuous-work-plan.md](parity/continuous-work-plan.md) | [Auditoría](audits/2026-09-18-direction.md); GitHub contiene el alcance de cada tarea |
| Madurez global, road y rail | [PARIDAD.md](PARIDAD.md) | Roadmaps y guía de mapa sólo enlazan este estado |
| Compatibilidad `.sav` import/export | [parity/sav-compatibility.md](parity/sav-compatibility.md) | [PLANIFICACION.md](PLANIFICACION.md#export-sav) explica el writer; [MAPA_Y_FERROCARRIL.md](MAPA_Y_FERROCARRIL.md) el formato/pipeline |
| NewGRF Action0/3/5 | [parity/newgrf-action0-matrix.md](parity/newgrf-action0-matrix.md) | Propiedades de catálogos y render |
| NewGRF callbacks | [parity/newgrf-callback-matrix.md](parity/newgrf-callback-matrix.md) | Call sites, storage y residual |
| NewGRF V1 CB36 | [parity/newgrf-cb36-v1.md](parity/newgrf-cb36-v1.md) | Fixture GRF V2, hash y recarga JSON certificada |
| Barco V1 | [parity/ship-delivery-v1.md](parity/ship-delivery-v1.md) | Entrega de carbón vía boya, conservación y hash determinista |
| Avión V1 | [parity/air-delivery-v1.md](parity/air-delivery-v1.md) | Servicio Country, bloques FTA y entrega de pasajeros |
| Late join TCP V1 | [parity/late-join-v1.md](parity/late-join-v1.md) | Dedicated, dos clientes, 2.000 ticks y hash/log por tick |
| GS JSON V1 | [parity/gs-json-v1.md](parity/gs-json-v1.md) | Meta Coal 5/10 → 10/10, dos compañías y noticia deduplicada |
| Editor V1 | [parity/editor-v1.md](parity/editor-v1.md) | Escenario 64×64 editado, JSON aislado y reapertura por Escenarios |
| Raster V1 de Kale | [parity/raster-v1-kale.md](parity/raster-v1-kale.md) | Contrato focal, hashes, presupuesto y artefactos de #589 |
| Reporte V1 por contrato | [parity/v1-report.md](parity/v1-report.md) | Agregado fail-closed por SHA, umbral y artefactos de los contratos V1 |

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
| [parity/newgrf-cb36-v1.md](parity/newgrf-cb36-v1.md) | Contrato focal CB36 de camión tras JSON |
| [parity/ship-delivery-v1.md](parity/ship-delivery-v1.md) | Contrato focal de entrega naval con boya |
| [parity/air-delivery-v1.md](parity/air-delivery-v1.md) | Contrato focal de servicio aéreo Country |
| [parity/late-join-v1.md](parity/late-join-v1.md) | Contrato focal TCP de late join con dos clientes |
| [parity/gs-json-v1.md](parity/gs-json-v1.md) | Contrato focal GS-lite de carga y recarga JSON |
| [parity/editor-v1.md](parity/editor-v1.md) | Contrato focal del editor, guardado JSON y reapertura por menú |
| [parity/raster-v1-kale.md](parity/raster-v1-kale.md) | Evidencia y reproducción del contrato raster focal V1 |
| [parity/v1-report.md](parity/v1-report.md) | Formato y agregado verificable de los resultados V1 por contrato |
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

*Última actualización: 2026-09-19 (corte completado; sin backlog activo)*
