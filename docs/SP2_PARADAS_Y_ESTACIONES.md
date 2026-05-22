# SP2 — Paradas de carretera y estaciones (referencia)

Documento de cierre para construcción en solitario: qué significa cada herramienta, sprites OpenTTD originales y qué pinta **openttdrs** hoy.

Relacionado: [PLAN_SP2_CONSTRUCCION.md](PLAN_SP2_CONSTRUCCION.md), [SPRITES_OPENGFX_COMPLETO.md](SPRITES_OPENGFX_COMPLETO.md) §6, [PLAN_SP3_VISUAL.md](PLAN_SP3_VISUAL.md), **[PLAN_PARADAS_REMAPCOORDS.md](PLAN_PARADAS_REMAPCOORDS.md)** (investigación `RemapCoords` + roadmap BUILD).

---

## 1. Tres tipos distintos (no confundir)

| Herramienta en toolbar | `BuildMenuAction` | `StopKind` / tesela | Familia OpenGFX |
|------------------------|-------------------|---------------------|-----------------|
| **Parada de bus** | `BusStop` | `Station` + `m6` tipo Bus | `SPR_BUS_STOP_*` (2692–2707) |
| **Estación** (panel carretera) | `Station` | `Station` + `m6` tipo Truck | `SPR_TRUCK_STOP_*` (2708–2723) |
| **Estación de tren** (panel vía) | `RailStation` | `Station` + `m6` tipo Rail | `SPR_RAIL_PLATFORM_*` (1069–1086) |

En OpenTTD son tres mecánicas distintas (`StationType::Bus`, `Truck`, estación de tren). El botón **«Estación»** del panel de carretera **no** es la estación de tren: es la **parada de carga** (camiones).

---

## 2. Nombres originales (OpenTTD / OpenGFX)

### Parada de bus — IDs 2692–2707

| ID | Constante |
|----|-----------|
| 2692 | `SPR_BUS_STOP_NE_GROUND` |
| 2693 | `SPR_BUS_STOP_SE_GROUND` |
| 2694 | `SPR_BUS_STOP_SW_GROUND` |
| 2695 | `SPR_BUS_STOP_NW_GROUND` |
| 2696–2699 | `SPR_BUS_STOP_*_BUILD_A` |
| 2700–2703 | `SPR_BUS_STOP_*_BUILD_B` |
| 2704–2707 | `SPR_BUS_STOP_*_BUILD_C` |

### Parada de camión («Estación» en UI) — IDs 2708–2723

| ID | Constante |
|----|-----------|
| 2708 | `SPR_TRUCK_STOP_NE_GROUND` |
| 2709 | `SPR_TRUCK_STOP_SE_GROUND` |
| 2710 | `SPR_TRUCK_STOP_SW_GROUND` |
| 2711 | `SPR_TRUCK_STOP_NW_GROUND` |
| 2712–2723 | `SPR_TRUCK_STOP_*_BUILD_A/B/C` |

### Estación de tren — IDs 1069–1086

Ejemplos: `SPR_RAIL_PLATFORM_X_FRONT` (1072), `SPR_RAIL_PLATFORM_BUILDING_X` (1073), techos 1079–1086. Ver [SPRITES_OPENGFX_COMPLETO.md](SPRITES_OPENGFX_COMPLETO.md) §5.

---

## 3. Archivos PNG en openttdrs

Generados por `scripts/descargar_graficos.sh`:

**Bus**

- `assets/opengfx/tiles/bus_stop_{ne,se,sw,nw}_ground.png` ← sprites 2692–2695
- `bus_stop_*_build_{a,b,c}.png` ← 2696–2707 (assets; render pendiente: ver nota abajo)

**Camión**

- `assets/opengfx/tiles/truck_stop_ground_{0..3}.png` ← alias de 2708–2711 (0=ne … 3=nw)
- `truck_stop_{dir}_ground.png` (+ `build_*` en assets; render solo GROUND por ahora)

**Tren**

- `assets/opengfx/tiles/rail_{1069..1074}.png` y capas en `sprites/station.rs` (`rail_station_draw_layers`)

---

## 4. ¿Mismo sprite bus vs «Estación»?

**No en OpenTTD** (IDs y arte distintos).

**En openttdrs** tampoco comparten el mismo PNG:

- Bus → `bus_stop_*_ground.png`
- Camión → `truck_stop_ground_*.png`

Si aún se ven parecidos, revisar orientación (`m5` bajo).

**Render mapa:** `*_ground.png` + `BUILD_A/B/C` (`RemapCoords` + offsets NFO) + tramo `road_flat` en `m3`.

**Preview construcción:** mismas capas en `ui/toolbar/preview/road_stop.rs` (1×1, sin halo 9×9).

Regenerar metadatos: `python3 scripts/gen_road_stop_gfx_data.py` — ver [PLAN_PARADAS_REMAPCOORDS.md](PLAN_PARADAS_REMAPCOORDS.md).

Código: `spawn_station_tile` → `spawn_stop_ground_sprite` + `spawn_road_stop_buildings`.

---

## 5. Conexión carretera ↔ parada (SP2 implementado)

OpenTTD une la parada a la red con road bits en la tesela de parada y en la carretera vecina (`MakeRoadStop`, `station_cmd.cpp`).

En openttdrs (`connect_road_stop` en `command/transport.rs`):

1. Tras colocar bus/camión, se fusionan **road bits** en la tesela de carretera adyacente (dirección `dir` de construcción, RMB rota).
2. En la tesela de parada, `m3` bajo guarda el tramo de acceso (bits hacia la carretera).
3. El render pinta ese tramo con `road_flat_*` bajo el sprite de parada.
4. El pathfinder trata la parada con `m3 != 0` como parte de la red de carretera.

Convención de bits: misma que `road_bits_for_render` (vecino en x−1 → bit 8, etc.). No reutilizar los bits de la boca del depósito en la tesela de parada (van en sentido opuesto).

**Depósito carretera:** sigue el patrón distinto de `road_depot_exit_for_dir` + `place_road_bits` en la tesela de salida.

---

## 6. Estación de tren (SP2 + SP3 parcial)

Colocación (`place_rail_station`):

- `m5` con `StationGfx` 2/3 (plataforma + edificio pequeño), eje alineado con `TrackBits` de la vía vecina.
- `mapt` estación (`0x50`), tipo en `m6`.

Render:

- Vía de fondo 1011/1012 + capas 1070–1074 con offsets de `station_land.h` (`rail_station_draw_layers`).

Pendiente SP3: estaciones multi-tesela, techos/pilares tren (gfx 4–7); preview ghost con edificios en toolbar.

---

## 7. Prueba rápida (mapa demo)

```bash
cargo run -p openttdrs-client
```

| Zona | Coordenadas (aprox.) |
|------|----------------------|
| Carretera demo | y=6, x=2..12 |
| Vía demo | y=14, x=2..12 |
| Canal puente | y=9..11, orillas x=3/15 |
| Túnel demo | NE (18, 8) |

Construcción: parada bus junto a carretera (RMB orienta boca), «Estación» camión aparte, estación tren en panel vía.

Fixture denso: `OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/sp3_visual_checklist.ottdmap cargo run -p openttdrs-client`
