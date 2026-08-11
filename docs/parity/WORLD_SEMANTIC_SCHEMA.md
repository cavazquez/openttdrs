# Contrato `world-semantic` v2

`world-semantic` es el segundo nivel de diagnóstico de una partida `.sav`.
`world-raw` responde si los bytes MAP* llegaron iguales; este contrato responde
si OpenTTD y `openttdrs` les asignan el mismo significado antes de dibujarlos.

Cada archivo es JSONL: una cabecera `metadata`, seguida de filas
`tile_semantic` en orden fila-mayor. El stream es deliberadamente local por
tesela, excepto `details.other_end`, que mide la topología que resuelve cada
motor para una rampa de túnel/puente.

## Cabecera

```json
{
  "kind": "metadata",
  "schema_version": 2,
  "contract": "world-semantic",
  "producer": "openttd",
  "stage": "after_load_game",
  "tick": 3703074,
  "climate": 0,
  "openttd_commit": "...",
  "source_path": "/ruta/partida.sav",
  "save_sha256": "...",
  "save_version": 0,
  "width": 256,
  "height": 256,
  "tile_count": 65536,
  "emitted_tile_count": 65536,
  "region": null
}
```

`producer` y `stage` describen el observador y no fallan por defecto en el
comparador. Dimensiones, región, contrato y el hash de la partida sí son
invariantes. `region`, cuando no es `null`, mantiene coordenadas e índices
absolutos e incluye límites inclusivos `min_x`, `min_y`, `max_x`, `max_y`.

## Fila común

```json
{
  "kind": "tile_semantic",
  "index": 22395,
  "x": 123,
  "y": 87,
  "tile_type": 9,
  "class": "tunnel_bridge",
  "tileh": 12,
  "base_z": 7,
  "owner": 2,
  "bridge_above_axis": null,
  "supported": true,
  "unsupported_reason": null,
  "raw": {"height": 8, "type": 144, "m1": 2, "m2": 0, "m3": 0, "m4": 0, "m5": 128, "m6": 0, "m7": 0, "m8": 2},
  "details": {"family": "tunnel_bridge", "...": "..."}
}
```

- `tile_type` es el nibble alto de MAPT (`TileType` de OpenTTD), no el byte
  MAPT entero.
- `tileh` y `base_z` son `GetTileSlopeZ` / `tile_slope_and_z`, no los bits
  bajos de MAPT. Esto evita confundir flags de puente/tropical con pendientes.
- `bridge_above_axis` es `0` para X, `1` para Y o `null`.
- `owner` es `null` donde OpenTTD no define `GetTileOwner` (casas, industria y
  void); en el resto es el owner sin flags (`m1 & 0x1f`).
- `supported=false` y `unsupported_reason` hacen visible un fallback, en vez
  de ocultarlo detrás de un sprite genérico.
- `raw` conserva MAPT/MAPH/MAPO/MAP2/M3LO/M3HI/MAP5/MAP6/MAP7/MAP8 para que una
  diferencia semántica siempre tenga contexto verificable.

## `details` por familia

| `class` | Campos relevantes |
| --- | --- |
| `clear` | `ground`, `density`, `counter`, `field_type`, `snow` |
| `railway` | `rail_tile_type`, `track_bits`, `rail_type`, `depot_direction`, `signal_present`, `signal_state`, `reservation_track_bits` |
| `road` | `road_tile_type`, `road_bits`, `tram_bits`, `road_type`, `tram_type`, ejes de cruce, `depot_direction`, `roadside` |
| `house` | `town_id`, `house_type`, `completed`, `building_stage` |
| `trees` | `tree_type`, `ground`, `density`, `count`, `growth`, `water_class` |
| `station` | ID/tipo/gfx, vía/eje/catenaria, spec, layout de parada, y parte/dirección de muelle |
| `water` | `water_tile_type`, `water_class`, eje/parte/dirección de depósito naval, esclusa |
| `industry` | ID, terminada, etapa de construcción, `gfx` limpio |
| `tunnel_bridge` | túnel/puente, transporte, dirección, `other_end`, tipo de puente, vía/carretera/tranvía y reserva |
| `object` | `object_id`, `object_type` resuelto desde `OBJS`/`OBTY`, `random` |
| `void` / `unknown` | `family` |

Los campos que no aplican se escriben como `null`, no se omiten. Eso permite
una comparación de estructura estable y evidencia si un fallback interpretó la
familia equivocada.

## Cambio v2

La v2 añade `details.object_type`. En un `MP_OBJECT`, `raw.m5` no es el tipo:
es el byte alto del `ObjectID` (`m2 | m5 << 16`). El exportador Rust conserva
ese byte sin modificar y transporta el pool `ObjectID → ObjectType` en el
footer `OBTY`; el oráculo C++ consulta el mismo pool de OpenTTD. Así una
discrepancia de tipo de objeto se detecta antes de llegar a sus sprites.

## Topología intencionalmente comparada

En la fila `tunnel_bridge`, OpenTTD usa `GetOtherTunnelBridgeEnd`. El lado de
`openttdrs` usa sus resolvedores en producción (`resolve_tunnel_end`,
`rail_bridge_other_end` o `road_bridge_other_end`). Por eso una diferencia en
`details.other_end` no es una discrepancia cosmética: demuestra que la misma
rampa no se enlaza al mismo extremo lógico. Es precisamente la evidencia para
errores donde las vías parecen cortarse antes de un túnel o un puente.

## Uso

```bash
./scripts/export_openttd_world_semantic.sh partida.sav /tmp/openttd.semantic.jsonl /ruta/OpenTTD/build/openttd
./scripts/export_openttdrs_world_semantic.sh partida.sav /tmp/openttdrs.semantic.jsonl
python3 scripts/compare_world_semantic.py /tmp/openttd.semantic.jsonl /tmp/openttdrs.semantic.jsonl \
  --json-report /tmp/semantic.json --show-inventory
```

Para un área visible: `--tile X,Y --radius N` en ambos dumpers. Para filtrar el
informe: `--only railway,tunnel_bridge` o `--where X,Y` (repetible). El reporte
agrega `field_difference_counts`, los inventarios `*_unsupported` y, con
`--show-inventory`, cantidades por familia, entidad lógica, variante y
orientación. Así el siguiente arreglo se decide por frecuencia y no solo por la
primera captura.
