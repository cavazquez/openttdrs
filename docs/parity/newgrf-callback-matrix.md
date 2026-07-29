# Matriz de callbacks NewGRF (CBID) — OpenTTD 15.3

Referencia: commit `14ec60f248547d4d062a1160f0fc26d742319888`,
`reference/openttd-upstream/src/newgrf_callbacks.h`.

Issue de runtime: [#228](https://github.com/cavazquez/openttdrs/issues/228).
Catálogos Action0/3/5: [`newgrf-action0-matrix.md`](newgrf-action0-matrix.md).

Estados:

- **soportado**: se ejecuta en un call site real; fallo → [`CALLBACK_FAILED`](../../crates/openttdrs-core/src/newgrf_sprites/model.rs) observable (no silencioso).
- **almacenado**: `callback_mask` / props parseadas y guardadas; **no** se ejecuta el CB.
- **OOS**: fuera del MVP #228 (residual documentado).

API común: `TrainSpriteGraphics::resolve_callback` / `resolve_callback_ctx`,
`resolve_vehicle_callback`, `writeback_vehicle_persistent_registers`
(`crates/openttdrs-core/src/newgrf_callback.rs`).

## Por feature

| Feature | CBID (ejemplos) | Estado | Notas |
|---|---|---|---|
| Stations (`04`) | `0x24` `CBID_STATION_BUILD_TILE_LAYOUT` | **soportado** | Call site: construcción `apply_station_build_tile_layout_callback` |
| Stations | `0x13` availability, `0x14` draw layout, `0x140`–`0x142` anim, `0x149` slope | **OOS** | Máscaras/consumidas en Action0; sin call sites |
| Vehicles (`00`–`03`) | `0x31` `CBID_VEHICLE_START_STOP_CHECK` | **soportado** | Call site: `toggle_vehicle_running_checked`; deniega → `NewGrfCallbackDenied` |
| Vehicles | `0x10`–`0x12`, `0x15`–`0x16`, `0x19`, `0x1D`, `0x23`, `0x2D`, `0x32`–`0x36`, … | **OOS** | Evaluador Action2 listo; sin call sites |
| Houses (`07`) | `0x17`, `0x1A`–`0x1C`, `0x1E`–`0x21`, `0x2A`, `0x2E`, `0x143`, `0x148`, … | **almacenado** | `HouseSpecDef.callback_mask`; sin ejecutar |
| Industry tiles (`09`) | `0x25`–`0x27`, `0x2B`–`0x2C`, `0x2F`–`0x30`, `0x3C`, … | **almacenado** | `IndustryTileSpecDef.callback_mask` |
| Industries (`0A`) | `0x22`, `0x28`–`0x29`, `0x35`, `0x37`–`0x3B`, `0x3D`, `0x14A`+, … | **almacenado** | `IndustrySpecDef.callback_mask` |
| Airport tiles (`11`) / Airports (`0D`) | anim / FTA-related | **almacenado** / **OOS** | Máscaras; FTA bloqueado (#260) |
| Canals (`05`) | `0x147` sprite offset | **almacenado** | `CanalSpecDef.callback_mask` |
| Objects / Cargoes / RoadStops / Types | varios | **OOS** | Sin ejecución de CB en este corte |
| Generic | `0x01` `CBID_RANDOM_TRIGGER` | **OOS** | Ver triggers abajo |

## Storage

| Tipo | Ciclo de vida | Estado MVP |
|---|---|---|
| Temporal (`7D` / `\2sto`) | Solo durante la evaluación Action2 | Descartado al terminar el ctx |
| Persistente (`7C` / `\2psto`) | Vehículo: `Vehicle.newgrf_persistent_regs` | Writeback tras CB; round-trip JSON save |
| Persistente no-vehículo (estación/industria/casa/…) | — | **OOS** |

## Triggers / random

| Pieza | Estado |
|---|---|
| Industry tile `m3` random bits + `m6` triggers (reseed) | **Existe** (`industry_random.rs`, P7): acumula trigger, reseedea `m3`, limpia bits |
| `ResolveRerandomisation` / Action2 random sprite groups por trigger | **Residual / OOS** |
| Vehicle/station random Action2 (`0x80`/`0x83`/`0x84`) en resolve de sprites | **Parcial** (eval con `random_bits`; sin reseed por trigger de gameplay) |
| `CBID_RANDOM_TRIGGER` genérico | **OOS** |

## Call sites soportados (checklist)

1. Stations CB24 — layout al construir.
2. Vehicles CB31 — start/stop check (+ writeback persistent regs).

## Residual explícito (no bloquea cierre MVP #228)

- Ejecutar el resto de CBs houses / airports / industries / objects.
- Scopes parent/relative completos.
- Triggers → Action2 random groups (`ResolveRerandomisation`).
- Storage persistente en features no-vehículo.
- Goldens tick-a-tick vs OpenTTD 15.3 para todos los features.
- Textos GRF de string (`0x40F` / `regs100`) en CB31: denegación genérica `NewGrfCallbackDenied`.
