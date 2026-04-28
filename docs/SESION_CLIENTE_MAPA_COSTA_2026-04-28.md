# Sesión cliente / mapa / costa (2026-04-28)

Resumen de **cambios de código**, **motivos**, **hallazgos** y **problemas** tratados en esta sesión (openttdrs-client + iso + datos de mapa).

---

## 1. Rendimiento y estructura del cliente

### 1.1 `VehicleIndex` (`main.rs`)

- **Qué:** recurso `HashMap<u32, usize>` (`Vehicle.id` → índice en `GameState::vehicles`).
- **Por qué:** `update_vehicles` hacía `find` por cada sprite → **O(V²)** por frame cuando hay muchos vehículos; con el mapa es **O(V)** por frame + **rebuild** `O(V)` solo cuando la sim avanza tick (`advance_sim`).
- **Startup:** `.init_resource::<VehicleIndex>()` y cadena `(setup, rebuild_vehicle_index, setup_tile_info_ui).chain()`.

### 1.2 Rejilla de mapa en `setup` (`main.rs`)

- **Qué:** antes del bucle por tesela, precálculo de `tileh_grid`, `base_z_grid` y `use_shore_grid` (agua que debe usar gráficos de costa por `m5` + `water_tile_touches_land`).
- **Por qué:** evitar repetir **varias lecturas** `map.get` + `compute_tileh` / `tile_min_corner_height` por celda; una pasada con `tile_slope_and_min_z` alimenta los arrays.
- **Bucle:** `idx = ty * mw + tx`; costa usa `tileh` de la rejilla + `infer_coast_tileh_when_flat` solo si `th == 0` en la rama costa (antes del arreglo de §3.3 la lógica de `th` evolucionó).

### 1.3 `spawn_batch` (`main.rs`)

- **Qué:** acumular y emitir `spawn_batch` para agua animada `(WaterTile, Sprite, Transform)`, costa `(Sprite, Transform)` y bosques `(Sprite, Transform)`.
- **Por qué:** menos overhead de comandos que miles de `spawn` sueltos en el setup; **tres lotes** porque Bevy exige bundles homogéneos por lote, no por “paralelismo en 3 hilos”.

### 1.4 `animate_water` (`main.rs`)

- **Qué:** culling opcional con cámara 2D: `Affine3A` inversa + `Rect` de proyección ortográfica; si la tesela queda fuera del rectángulo visible (+ margen), no se actualiza el tinte.
- **Nota:** se corrigió el uso de referencias del iterador de cámara guardando datos **poseídos** (`world_to_view`, `area`).

### 1.5 Imports / `iso.rs`

- `tile_min_corner_height` delega en `tile_slope_and_min_z(...).1` (una sola pasada de esquinas; coherente con OpenTTD `GetTileZ` / `GetTileSlopeZ`).

---

## 2. Agua, alturas y costa (`iso.rs` + `main.rs`)

### 2.1 Inferencia de altura en muestras de agua/void (`height_for_slope_corner_sample`)

- **Problema:** en `.ottdmap` el **MAPH** en `MP_WATER` a menudo es **0** aunque el mar comparta nivel con la costa; usar `0` literal en el cuarteto 2×2 de `GetTileSlopeZ` hundía `min_h` y abría **huecos** / costa rota frente a la hierba.
- **Primera solución:** para celdas **Water/Void**, sustituir la altura de muestra por una inferida desde teselas de **tierra** en el vecindario de **8** direcciones.
- **Error con `max`:** en bahías o esquinas entrantes, cada celda de agua podía heredar **máximos distintos** → cuatro esquinas inconsistentes → **pendientes falsas** en agua (p. ej. `tileh` 13) y costas **asimétricas** respecto a la hierba vecina.
- **Corrección:** usar **`min`** entre alturas `Tile.height` de tierras vecinas (8 vecinos), no `max`. Objetivo: unificar mejor el “nivel del mar” entre celdas de agua contiguas.

### 2.2 MP_WATER en la UI vs costa real

- **Problema:** forzar **`tileh = 0`** en `tile_slope_and_min_z` para MP_WATER (correcto para no mostrar pendiente de terreno en agua en la UI) hacía que en el render la costa usara **solo** `infer_coast_tileh_when_flat`, que prioriza **un** vecino de tierra y devuelve sobre todo **W/E/S/N** (pendientes simples).
- **Efecto:** en costas **diagonales**, OpenTTD usa la pendiente **real** del 2×2 (`DrawShoreTile(GetTileSlopeZ)`), p. ej. **SW (3)**; con solo infer se elegía a menudo **W (1)** → **sierra** / sprites de costa incorrectos.
- **Corrección:** extraer `tile_slope_bits_from_heights` + **`shore_tileh_for_draw_shore`**: si el 2×2 no es plano, usar pendiente cruda; si es plano, `infer_coast`; para **WE (5)** y **NS (10)** OpenTTD no tiene sprite de costa dedicado → caer en infer (como `water_cmd.cpp` con asserts).
- **`main.rs`:** la rama costa usa `shore_tileh_for_draw_shore` en lugar de `tileh` del grid + infer.

### 2.3 Alineación vertical y orden Z

- **Problema A:** usar **mediana** de las cuatro esquinas como `min_z` en agua y **`min_h`** en tierra desalineaba **GetTileZ** entre hierba y agua → escalones visibles.
- **Corrección A:** **`min_z = min_h`** también para MP_WATER (igual que OpenTTD `GetTileZ`).

- **Problema B:** la Z del sprite incluye `(tx + ty) * 0.01`; el mar **al este/sur** tiene suma mayor y se dibujaba **encima** del borde costero del vecino → rectángulos azules / sierra.
- **Corrección B:** constante **`FLAT_WATER_LAYER_FRAC`** (valor negativo, p. ej. `-0.014`) solo en **`tile_pos` del agua animada** (no en `shore_*`).

---

## 3. Pendientes y convenciones (explicación, no bug de código)

### 3.1 Etiqueta de pendiente “NWE” vs “NW”

- **13** en bitmask OpenTTD = esquinas **N+W+E** por encima del mínimo (tres bits); **NW** es **9** (solo N+W).
- Confusión habitual: las letras listan **esquinas altas**, no “dirección de la pendiente a ojo”.

### 3.2 Dos teselas “parecidas a la vista” no son la misma celda ni simétricas en índice

- Ejemplo real: mapa **256×256**, teselas `(160,232)` y `(145,213)`.
- **Rotación 180° en índices:** `(x',y') = ((W-1)-x, (H-1)-y)` → `(145,213)` va a **`(110,42)`**, no a `(160,232)`.
- **Dump `.ottdmap`:** casi idénticas (mismo `mapt`, `m5`, `m1`…); la única diferencia en el binario fue **`height` 0 vs 1** — el save no las trata como clones; la vista isométrica puede engañar.

---

## 4. Herramientas añadidas

### 4.1 `dump_map_tiles` (ejemplo Rust)

- **Ruta:** `crates/openttdrs-core/examples/dump_map_tiles.rs`
- **Uso:**  
  `cargo run -p openttdrs-core --example dump_map_tiles -- <mapa.ottdmap> [tx ty ...]`
- **Qué imprime:** `W×H`, transformación 180° de ejemplo `(145,213)` con esas dimensiones, y por cada coordenada los campos de `Tile` (kind derivado, `height`, `mapt`, `m5`, `m1`, `m6`, `m8`, y `WaterTileType` si aplica).

### 4.2 Comparación `.sav` vs `.ottdmap` (pendiente de producto)

- En la sesión se analizó **`parse_sav.py`** como fuente de verdad del export; **no** se llegó a mergear un script `compare_sav_ottdmap_tiles.py` separado. Si se desea, el siguiente paso sería refactorizar la extracción en `parse_sav.py` a una función reutilizable y un CLI que compare byte a byte (o por coordenadas) el payload exportado con un `.ottdmap` existente.

---

## 5. Referencias OpenTTD útiles (lectura upstream)

| Tema | Archivo / símbolo (upstream) |
|------|--------------------------------|
| Pendiente y `min` de esquinas | `tile_map.cpp` — `GetTileSlopeZ`, `GetTileSlopeGivenHeight`, `GetTileZ` |
| Sprite de costa | `water_cmd.cpp` — `DrawShoreTile`, tabla `tileh_to_shoresprite[]` |
| Carga de mapa / chunks save | `map_sl.cpp` — nombres `MAPT`, `MAPH`, `MAP5`, `MAPO` (= MAP1), `MAPE`, `MAP8` |
| Export en este repo | `scripts/parse_sav.py` — docstring del formato `.ottdmap` v3 |

---

## 6. Tests añadidos o relevantes (cliente)

- Regresión **agua / costa / alturas:** `water_coast_height_tests` en `iso.rs` (península, charco anillo, `mp_water_never_exposes_terrain_slope_bits`, `shore_tileh_uses_diagonal_slope_not_infer_priority_w`).
- Los tests existentes de `compute_tileh` / `infer_coast` / `tile_min_corner_height` siguen pasando tras los cambios.

---

## 7. Comandos útiles

```bash
cargo test -p openttdrs-client
cargo run -p openttdrs-core --example dump_map_tiles -- assets/maps/mapa.ottdmap 160 232 145 213
python3 scripts/parse_sav.py partida.sav salida.ottdmap
```

---

## 8. Riesgos / seguimiento

- **`FLAT_WATER_LAYER_FRAC`:** si en mapas enormes o con muchas capas sigue habiendo solapamiento, puede afinarse (o plantear orden de dibujo tipo painter isométrico más estricto).
- **Inferencia `min` en agua:** en escenarios raros (acantilado + agua entre dos tierras a alturas muy distintas) podría necesitar otra heurística; hoy prioriza coherencia de costa típica.
- **Comparación `.sav` vs `.ottdmap`:** automatizar en script evita dudas de export para depuración futura.
