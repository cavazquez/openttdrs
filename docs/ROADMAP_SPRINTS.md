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
| **S3** | Visual ferrocarril | ✅ Pendientes/junctions slope; culling; industrias 0–174 |
| **S4** | SP1 ciclo jugable | Sesión 15–30 min sin pasos raros |
| **S5** | Señales v1 + audio | Trenes con bloques simples; SFX básicos |
| **S6** | Import `.sav` + órdenes | Partidas OTTD más jugables; full load básico |

---

## Sprint 1 — SP4: pulido y confianza ✅ (cerrado 2026-06-22)

**Objetivo:** guardar/cargar y CI dan seguridad para iterar rápido.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| Migración save real al cambiar esquema (bump v4 + test roundtrip) | S | `save.rs` + test v3→v4 ✅ |
| Test `effective_road_bits` en fixture `.ottdmap` | S | Regresión carreteras importadas ✅ |
| Cerrar checklist SP2 manual pendiente (1 pasada) | S | `SP2_CHECKLIST.md` § S1 refresh ✅ |
| Documentar flujo `check.sh ci` en README si falta | S | README ✅ |

**Done:** `bash scripts/check.sh` verde; migración probada; golden `parse_sav` OK.

---

## Sprint 2 — Toolbar ferroviario completo

**Objetivo:** los botones stub del toolbar rail hacen algo útil.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| `RailRemove`: borrar solo vía + `refresh_rail_neighbors` | S | ✅ `Command::RemoveRail` + preview |
| `RailWaypoint`: tesela waypoint + orden “pasar por” | S–M | ✅ `PlaceRailWaypoint` + render ogfx2 |
| Depósito carretera: calibración RemapCoords | S | ✅ Hecho — `gen_road_depot_gfx_data.py`, `road_depot_build_sprite_center` |
| `RailConvert`: diferir o railtypes mínimos (normal/eléctrico) | S–M | Stub oculto en toolbar hasta railtypes |

**Done:** construir T, quitar tramo, waypoint visitable por tren; depósito carretera alineado. **Resto:** solo `RailConvert`.

---

## Sprint 3 — Visual ferrocarril y mapas grandes ✅ (cerrado 2026-07)

**Objetivo:** mapa legible en pendientes; FPS estable en mapas grandes.

| Tarea | Costo | Entregable |
|-------|-------|------------|
| Junctions de vía en pendiente (overlays slope) | M | ✅ `sp3_visual_checklist_sloped_junction_sprite_ids` |
| Culling de teselas fuera de viewport (no solo agua) | M | ✅ `MapTileSpawnViewport` + `resync_town_labels` |
| Industrias gfx 120–174 (tabla vanilla) | M | ✅ `INDUSTRY_GFX_TABLE_LEN=175` + checklist y=10 |
| Captura regresión cruce X\|Y + slope | S | Driver `OPENTTDRS_MAP_SHOT_*` (CI opcional) |

**Done:** curva/cruce en colina correctos; mapa 256×256 con culling; industrias vanilla 0–174.

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

Referencia detallada (tipos oficiales, codificación `m2`/`m3`, fases A–E):
[SENALES_FERROVIARIAS.md](SENALES_FERROVIARIAS.md).

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

## Después de S6 — paridad estructural (hitos 0.2–0.6)

Detalle vivo: [ROADMAP_PARIDAD_ESTRUCTURAL.md](ROADMAP_PARIDAD_ESTRUCTURAL.md).

| Hito | Fase | Cuándo |
|------|------|--------|
| 0.2 | **Fase 1 Consist** ✅ + **Fase 2** cargo packets / rating ✅ | post-0.1 inmediato |
| 0.3 | **Fase 3** YAPF incremental + PBS multi-tesela ✅ (MVP) | tras consist estable |
| 0.4 | **Fase 4** economía multi-compañía + feeder + IA ✅ (MVP) | tras packets |
| 0.5 | **Fase 5–6** railtypes + mono/maglev ✅ (MVP) | tras PBS básico |
| 0.6 | **Fase 7** NewGRF config ✅ (MVP) → runtime Action0–14 | meta larga |

| Item | Cuándo |
|------|--------|
| PBS / path signals | Hito 0.3 (Fase 3) |
| Barcos / aviones | paralelo / hito transporte |
| Terraform (elevar / bajar / nivelar) | [ROADMAP_TERRAFORM.md](ROADMAP_TERRAFORM.md) T1–T3; paralelo a S4 |
| Barra de estado + noticias | [ROADMAP_NEWS_STATUSBAR.md](ROADMAP_NEWS_STATUSBAR.md) N1–N3; mejora SP1 |
| Generación de mundo + 4 climas | Hito 0.2+ |
| Cargo Dist / link graph | Hito 0.4 (Fase 4) |
| Multijugador I8 | Explícitamente post-paridad de sim |
| NewGRF runtime | Hito 0.6 (Fase 7) |
| Flota F0–F8 (timetable, autoreemplazo, pool) | [ROADMAP_MENUS_UI.md](ROADMAP_MENUS_UI.md) §13 |
| **Paridad UI global** (toolbar, menús, directorios, ventanas) | [ROADMAP_PARIDAD_UI_GLOBAL.md](ROADMAP_PARIDAD_UI_GLOBAL.md) UI-0–UI-8 |
| **Junctionary completo** (cruces comunidad OTTD) | [ROADMAP_JUNCTIONARY.md](ROADMAP_JUNCTIONARY.md) J0–J5 |

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

*Última actualización: 2026-06-22*
