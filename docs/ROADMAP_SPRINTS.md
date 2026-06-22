# Roadmap por sprints — hito 0.1 (solitario)

Plan operativo en **6 sprints** (~2 semanas c/u). Solo ítems de costo **S–M** (días a ~2 semanas).
Objetivo: cerrar el **vertical slice en solitario** sin abrir multijugador, NewGRF runtime ni Cargo Dist.

**Relacionado:** [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) (tabla completa tenemos / falta),
[SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md) (comandos y hallazgos técnicos),
[DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) (spec I0–I8).

---

## Visión por sprint

| Sprint | Foco | Resultado jugable |
|--------|------|-------------------|
| **S1** | SP4 + deuda técnica | Saves migrables; tests de regresión mapa |
| **S2** | Toolbar ferroviario | Quitar vía, waypoint, depósito carretera OK |
| **S3** | Visual ferrocarril | Pendientes y junctions en slope; culling básico |
| **S4** | SP1 ciclo jugable | Sesión 15–30 min sin pasos raros |
| **S5** | Señales v1 + audio | Trenes con bloques simples; SFX básicos |
| **S6** | Import `.sav` + órdenes | Partidas OTTD más jugables; full load básico |

---

## Sprint 1 — SP4: pulido y confianza

**Objetivo:** guardar/cargar y CI dan seguridad para iterar rápido.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| Migración save real al cambiar esquema (bump v4 + test roundtrip) | S | `save.rs` + test v3→v4 |
| Test `effective_road_bits` en fixture `.ottdmap` | S | Regresión carreteras importadas |
| Cerrar checklist SP2 manual pendiente (1 pasada) | S | `SP2_CHECKLIST.md` |
| Documentar flujo `check.sh ci` en README si falta | S | README |

**Done:** `bash scripts/check.sh` verde; migración probada; golden `parse_sav` OK.

---

## Sprint 2 — Toolbar ferroviario completo

**Objetivo:** los botones stub del toolbar rail hacen algo útil.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| `RailRemove`: borrar solo vía + `refresh_rail_neighbors` | S | Comando + preview |
| `RailWaypoint`: tesela waypoint + orden “pasar por” | S–M | Pathfinding lo atraviesa |
| Depósito carretera: calibración RemapCoords | S | Ver `archive/PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md` |
| `RailConvert`: diferir o railtypes mínimos (normal/eléctrico) | S–M | Tooltip honesto o implementación mínima |

**Done:** construir T, quitar tramo, waypoint visitable por tren; depósito carretera alineado.

---

## Sprint 3 — Visual ferrocarril y mapas grandes

**Objetivo:** mapa legible en pendientes; FPS estable en mapas grandes.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| Junctions de vía en pendiente (overlays slope) | M | `sprites/rail.rs` + `MAP_SHOT` |
| Culling de teselas fuera de viewport (no solo agua) | M | `render/world.rs` |
| Industrias gfx 120–130 (frecuentes en saves) | M | `ROADMAP_INDUSTRIAS_PARIDAD.md` |
| Captura regresión cruce X\|Y + slope | S | `OPENTTDRS_MAP_SHOT_*` |

**Done:** curva/cruce en colina correctos; mapa 256×256 con FPS razonable.

---

## Sprint 4 — SP1: ciclo jugable cerrado

**Objetivo:** partida de **15–30 minutos** sin trucos manuales.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| Checklist SP1: industria → estación → vehículo → carga → pago | M | Test integración + doc |
| HUD: “sin ruta”, “sin carga”, estación incompatible | S | `hud/display/` |
| Coherencia `state.stations` ↔ tiles `MP_STATION` | M | Tests `state/stations.rs` |
| SFX: construcción, error, ingreso cargo (3–5 samples) | S | Bevy audio |

**Guion manual (15 min):**
1. Mina + fábrica (o mapa demo).
2. Estación camión + ruta con 2 paradas.
3. Estación tren 3×2 + tren + 2 órdenes.
4. Ver carga y dinero en HUD.
5. F5 guardar → reiniciar → F9 cargar.

---

## Sprint 5 — Señales v1 (bloque simple) + audio

**Objetivo:** primer paso hacia ferrocarril “serio” sin PBS completo.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| `Command::PlaceRailSignal` (bloque eléctrica) | M | Bits en tile |
| Sim: un tren por bloque (reserva hasta salir) | M | `sim_step` |
| Toolbar señales conectado al comando | S | Ya hay icono |
| Preview fantasma señal | S | Como autorail |
| Música ambiente (1 track, opcional) | S | Toggle en menú |

**Fuera de alcance S5:** presignals, path signals, PBS, YAPF.

---

## Sprint 6 — Import `.sav` jugable + órdenes básicas

**Objetivo:** abrir save OpenTTD y jugar algo reconocible.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| Import: dinero, tick, órdenes VEHS básicas | M | `sav/entities` → `Vehicle.orders` |
| Orden Full load / no unload (2 flags) | M | Extender `VehicleOrder` |
| Panel órdenes: tipo de parada | S | `order_panel` |
| Test: fixture save + sim 100 ticks | M | `tests/sav_load.rs` |
| Doc limitaciones import en `TILES_Y_SAVEGAMES_OPENTTD.md` | S | § limitaciones |

---

## Después de S6 (backlog L+, post-0.1)

| Item | Cuándo |
|------|--------|
| PBS / path signals | Hito 0.2 ferro avanzado |
| Barcos / aviones | Hito 0.3 transporte completo |
| Generación de mundo + 4 climas | Hito 0.2 |
| Cargo Dist / link graph | Muy post-0.1 |
| Multijugador I8 | Explícitamente post-0.1 |
| NewGRF runtime | Opcional / largo plazo |

---

## Dependencias

```
S1 (SP4) ──┬──► S2 (toolbar rail) ──► S5 (señales)
           ├──► S3 (visual) ──► S4 (SP1 ciclo)
           └──► S4 ──► S6 (import sav)
```

S2 y S3 pueden ir en paralelo.

---

## Criterios de cierre hito 0.1

- [ ] Sesión solitario 15–30 min sin bugs bloqueantes
- [ ] Guardar/cargar JSON con migraciones
- [ ] Construcción road + rail completa (menos convert)
- [ ] Estación tren multi-tesela + ventana selección
- [ ] Señales bloque básicas
- [ ] Import `.sav` con vehículos que se mueven
- [ ] `check.sh` + CI verdes
- [ ] Paridad visual SP3 ≥ 90 % (`ROADMAP_PARIDAD_VISUAL.md`)

---

*Última actualización: 2026-06-11*
