# Sesión: `.ottdmap` v5+12, señales, cruces, simulación y persistencia (2026-04-28)

Documento de referencia de los cambios integrados en `main` en esta línea de trabajo (cliente + core + scripts).

## Formato `.ottdmap`

### Planos densos

- **v5 (11 planos por tesela):** `MAPT`, `MAPH`, `MAP5`, `MAPO`, `MAPE`, `MAP8` (LE), `M3LO`, `MAP2` (byte **bajo** de `m2()`), `MAP7`, `M3HI`.
- **v5+12 (12 planos):** tras `M3HI` se añade un byte por tesela = **byte alto de MAP2** (`m2()` 16-bit en el save OpenTTD, `map_sl.cpp`). Sirve para bits altos (p. ej. reserva PBS en vía).

### Semántica OpenTTD importante

- El chunk save **`M3HI` carga en `m4()`** del mapa OpenTTD, no es el “byte alto de m3”. En el cliente se sigue llamando campo `m3hi` en `Tile`, pero corresponde a **`m4()`** (p. ej. nibble alto para `GetSignalStates` en señales).
- **`dense_payload_end(data, n)`** (Rust): recibe el buffer completo; si en el offset `12 + 11·n` empieza un magic de footer conocido (`INDP`, `STNN`, `TNBP`, `M2HI`), el denso termina en **11 planos**. Si no hay footer ahí pero `len ≥ 12 + 12·n`, se asume plano **`m2_hi`** incluido.

### Footers

- **`INDP`:** `u32` count + `count × (u16 industry_index, u8 industry_type)`.
- **`STNN` / `TNBP`:** `u32` length + blob (pool estaciones / túnel-puente); el core los guarda en `OttdmapExtras`; el cliente solo loguea tamaños (parse estructurado pendiente).

### Export Python (`scripts/parse_sav.py`)

- Si `len(MAP2) ≥ 2·W·H`, exporta MAP2 como **u16 LE** (bajo + alto); si no, bajo + relleno de ceros en alto.
- El cuerpo del `.ottdmap` pasa a **12 planos** cuando hay fila `m2_hi` (incluido todo ceros), ~**+W×H bytes** respecto al layout de 11 planos.

## Core (`openttdrs-core`)

- **`ottdmap_extras.rs`:** `OttdmapExtras`, `parse_footers`, `industry_type_for_tile_index`, tests.
- **`Map::from_ottd_binary_with_extras`:** mapa + extras; `dense_payload_end` alineado con footers.
- **`Tile`:** campo **`m2_hi`**; documentación de `m3hi` ↔ `m4()`.
- **`SimStats`:** `cargo_pickups`, `cargo_deliveries`, `cargo_units_loaded`, `cargo_units_delivered`, `industry_cargo_units_produced`; actualizado en `GameState::step`.
- **Persistencia:** `serde` / `serde_json`; `GameState::save_json` / `load_json`; derives en tipos de estado, `GameTick` transparente.

## Cliente (`openttdrs-client`)

### Gráficos y vía

- Sprites de señal: **`collect_signal_sprite_ids`**, máscaras presente/estado desde `m3` / `m3hi` (nibble `m4`), fórmula de sprite con base **1275** (bloque eléctrico clásico) y base alternativa **1352** para el resto (aprox. OpenGFX 8bpp).
- Texturas cargadas **`rail_1275.png` … `rail_1519.png`** (rango ampliado en `descargar_graficos.sh`).
- **Cruce a nivel:** sprite +2 si barrera (`m5` bit 5); tinte si reserva (bit 4); tinte suave si hay tranvía (`m8`).
- Título de ventana: tick, cargas, contador INDP si aplica.

### Estado y mapa cargado

- **`SimWorld`:** `ottdmap_extras: Option<OttdmapExtras>`; carga con `from_ottd_binary_with_extras`.
- **`place_industries(..., Option<&OttdmapExtras>)`:** tipo de industria vía footer `INDP` + mapeo OpenTTD → `IndustryKind`.
- **`place_stations_from_map_tiles`:** estaciones simuladas en teselas **`MP_STATION`** (deduplicado).
- Orden al cargar archivo: industrias → estaciones junto a industrias → estaciones desde mapa → vehículos.
- **`log_detection_summary`:** tercer argumento `extras` para log de footers.

## Scripts

- **`descargar_graficos.sh`:** `rail_1372`/`1373` explícitos; bucle **`1275..1520`** para señales extendidas. Requiere **`grfcodec`** instalado para extraer PNG.

## Tests

- Core: `from_ottd_binary_loads_m2_hi_plane`, `dense_end_12_planes_before_indp`, `parses_indp_after_v5_dense`, `game_state_json_roundtrip`, `sim_stats_count_pickup_and_delivery`, etc.
- Cliente: cruces barrados, recolección de sprites de señal, variante semáforo vs eléctrico.

## Cómo verificar localmente

```bash
cargo test
cargo clippy -p openttdrs-core -p openttdrs-client -- -D warnings
python3 scripts/verify_parse_sav_reference.py
```

Tras tocar `parse_sav.py` de forma que cambie estadísticas del golden, regenerar con `scripts/emit_parse_sav_golden.py` según `README.md`.

## Próximos pasos sugeridos (no implementados aquí)

1. Parse real de **STNN** (estaciones / waypoints) y uso en simulación sin depender solo de `MP_STATION`.
2. **PBS / reserva** en renderer de vía usando `m2_hi` + bits de reserva en `m2` completo.
3. **CI / artefactos:** cache de `grfcodec` o job que genere tiles para pruebas visuales.
4. **Hotkeys o CLI** para `save_json` / `load_json` desde el cliente.
5. Afinar **`SPR_SIGNAL_ALT_BASE`** contra un juego de referencia o tabla GRF extraída.
