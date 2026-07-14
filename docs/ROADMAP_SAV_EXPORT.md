# Export `.sav` OpenTTD — handoff para otra IA

Documento de reproducción: cómo guardar y cargar el mismo formato `.sav` en
openttdrs, qué está implementado, qué falta, y cómo extenderlo sin romper el
import existente.

**Estado (2026-07-08):** export operativo (mapa + `STNN` + `CITY` + `INDY` + `ORDL` + `VEHS` + `DATE` + `PLYR`).
El JSON propio sigue siendo el formato más completo (horarios, grupos, shared orders, etc.).

---

## 1. Objetivo

| Acción | Formato | API |
|--------|--------|-----|
| Cargar | `.sav` OpenTTD **o** `.json` propio | `sav::load` / `save::load_from_str` |
| Guardar (UI, por defecto) | `.sav` (OTTZ) | `sav::save` |
| Guardar (sim extendida) | `.json` (sufijo explícito) | `save::save` |

La UI de partidas (`ui/save_window/`):

- Nombre sin extensión → escribe `{nombre}.sav`.
- Sufijo `.sav` → export OpenTTD (mapa + entidades mínimas).
- Sufijo `.json` → save nativo (horarios, grupos, shared orders, etc.).

---

## 2. Cómo reproducir / verificar

Desde la raíz del repo `openttdrs/`:

```bash
# Tests del writer + roundtrip
cargo test -p openttdrs-core sav::write::

# Suite habitual
bash scripts/check.sh

# Smoke manual (cliente)
# 1. Nueva partida / cargar mapa
# 2. F5 → nombre "prueba" → Guardar → debe crear save/prueba.sav
# 3. F9 → elegir prueba.sav → Cargar
# 4. Para JSON completo: guardar como "prueba.json"
```

Roundtrip programático:

```rust
use openttdrs_core::{sav, GameState};

let state = GameState::new(64, 64);
let bytes = sav::save_to_bytes(&state)?;           // OTTZ
let bytes = sav::save_to_bytes_with(&state, sav::SavContainer::Ottn)?; // tests
let loaded = GameState::from_sav_game(sav::load(&bytes)?);
```

Referencia sintética de chunks (solo lectura / fixtures):

```bash
python3 scripts/gen_demo_sav.py crates/openttdrs-core/tests/fixtures/demo_openttd.sav
```

---

## 3. Archivos clave

| Ruta | Rol |
|------|-----|
| `crates/openttdrs-core/src/sav/write.rs` | **Writer**: planos + STNN/CITY/INDY/ORDL/VEHS + DATE + PLYR |
| `crates/openttdrs-core/src/sav/mod.rs` | `load`, `SavError`, reexport `save` / `SavContainer` |
| `crates/openttdrs-core/src/sav/container.rs` | OTTN / OTTZ / OTTX decompress |
| `crates/openttdrs-core/src/sav/chunks.rs` | Parse RIFF / TABLE |
| `crates/openttdrs-core/src/sav/build.rs` | Chunks → `.ottdmap` en memoria |
| `crates/openttdrs-core/src/sav/date.rs` | Lectura `DATE` |
| `crates/openttdrs-core/src/sav/entities.rs` | Lectura `PLYR` / STNN / CITY / INDY / VEHS |
| `crates/openttdrs-core/src/sav/orders.rs` | Lectura ORDL / ORDR |
| `crates/openttdrs-core/src/save.rs` | Persistencia **JSON** (no confundir) |
| `crates/openttdrs-client/src/ui/save_window/systems.rs` | `confirm_save` / `confirm_load` |
| `scripts/gen_demo_sav.py` | Generador OTTN de referencia |
| `docs/TILES_Y_SAVEGAMES_OPENTTD.md` §16–17 | Formato chunks / import |

Versión de export: `EXPORT_SAVE_VERSION = 350` (≥ 348 HouseID en MAP8; ≥ 300 tick u64).

---

## 4. Formato escrito hoy

### Contenedor

```
OTTZ | OTTN
u16 BE version (= 350)
u16 BE unused (= 0)
payload (zlib si OTTZ; raw si OTTN)
```

### Stream de chunks (orden)

1. `MAPS` — `CH_RIFF`, 8 bytes: `dim_x`, `dim_y` **big-endian** u32  
2. Planos `CH_RIFF` densos (W×H bytes, salvo MAP2/MAP8 = 2×):
   - `MAPT`, `MAPH`, `MAPO` (m1), `MAP2` (u16 BE: hi=`m2_hi`, lo=`m2`),
   - `M3LO`, `M3HI` (= m4 OpenTTD), `MAP5`, `MAPE` (m6), `MAP7`, `MAP8` (u16 BE desde `Tile.m8` LE)
3. `STNN` — `CH_TABLE` `xy` / `name` / `facilities` desde `GameState.stations`
4. `CITY` — `CH_TABLE` `xy` / `name` / `cache.population` / townname* desde `GameState.towns`
5. `INDY` — `CH_TABLE` `location.tile` / `w` / `h` / `type` desde `GameState.industries`
6. `ORDL` — `CH_TABLE` con struct `orders` (goto estación/waypoint); una lista por vehículo con órdenes
7. `VEHS` — `CH_SPARSE_TABLE` cabezas tren/bus/camión + ref a ORDL
8. `DATE` — `CH_TABLE` `date` (i32) + `tick_counter` (u64)
9. `PLYR` — `CH_TABLE` `money` (i64) + `colour` (u8)
10. Terminador `00 00 00 00`

### Mapeo `Tile` → planos

- Si `tile.mapt != 0` → se escribe tal cual.
- Si `mapt == 0` → se deriva del `TileKind` (`0x10` rail, `0x20` road, `0x50` station, `0x90` tunnel/bridge, …). Ver `tile_mapt()` en `write.rs`.

Endianness crítica (debe coincidir con `build.rs` al importar):

- `MAP2` save = BE → en memoria `m2_hi` / `m2`
- `MAP8` save = BE → en memoria `m8` LE

---

## 5. Limitaciones (no romper expectativas)

| Chunk / dato | Estado |
|--------------|--------|
| Planos + DATE + PLYR | ✅ |
| `STNN` | ✅ nombres + facilities |
| `CITY` | ✅ nombre + pos; población se recalcula al load |
| `INDY` | ✅ tile/w/h/type (mapeo `IndustrySpec` → tipo OTTD best-effort) |
| `VEHS` / `ORDL` | ✅ tren/bus/camión + goto estación/waypoint/depósito/condicional + full_load |
| Barcos / aviones | ❌ omitidos |
| Horarios / grupos / shared orders / autoreplace | ❌ solo en `.json` |
| `OBJS`, `NEWS`, settings, NewGRF | ❌ |

Por eso:

- Para **horarios, grupos, shared orders** → seguir usando `.json`.
- Para **mapa + estaciones + ciudades + flota básica** → `.sav` ya roundtrippea con `sav::load`.
- Abrir el `.sav` en OpenTTD oficial puede fallar (faltan settings/NewGRF/chunks de juego completo). Objetivo: **roundtrip con nuestro loader**.

Fecha de calendario en `DATE`: aproximación `year * 365 + (doy - 1)`; el tick monotónico se preserva exactamente.

---

## 6. Extender el export (guía para la siguiente IA)

Orden sugerido:

1. ~~**`STNN`**~~ ✅  
2. ~~**`CITY`**~~ ✅  
3. ~~**`INDY`**~~ ✅  
4. ~~**`ORDL` + `VEHS`**~~ ✅ (goto estación/waypoint/depósito/condicional + full_load)  
5. ~~Órdenes depósito / condicionales / flags full_load más fieles~~ ✅  
6. Validar con OpenTTD real (settings + chunks obligatorios del juego completo)

Reglas:

- No cambiar el layout de planos que `build::export_ottdmap` ya asume.
- Preferir `CH_TABLE` SLV ≥ 295 (como el demo).
- Mantener `EXPORT_SAVE_VERSION` alineada con lo que el loader ya soporta.
- Tras cada chunk nuevo: `cargo test -p openttdrs-core sav::` y `bash scripts/check.sh`.
- Actualizar esta tabla y `TILES_Y_SAVEGAMES_OPENTTD.md` §17.

---

## 7. Errores frecuentes

| Síntoma | Causa probable |
|---------|----------------|
| `MAP2`/`m2_hi` invertidos tras roundtrip | Escribir MAP2 en LE en vez de BE |
| `m8` HouseID basura | MAP8 no en BE, o versión &lt; 348 |
| Dinero `None` al cargar | Falta chunk `PLYR` o tipo SLE distinto de 7 |
| UI guarda JSON sin querer | Nombre con `.json`; por defecto ya es `.sav` |
| Órdenes depósito/condicionales perdidas | ✅ `OT_GOTO_DEPOT` / `OT_CONDITIONAL` en export/import |
| Vehículos sin lista | Órdenes vacías → `orders` ref = 0 (válido) |

---

## 8. Relación con otros docs

- Import detallado: [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) §16–17  
- Paridad producto: [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) (fila Save)  
- Menú / cargar: [ROADMAP_MAIN_MENU.md](ROADMAP_MAIN_MENU.md)  
- Fixture demo: `scripts/gen_demo_sav.py`

---

*Última actualización: 2026-07-08 — export STNN+CITY+INDY+ORDL+VEHS.*
