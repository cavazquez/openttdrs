# `world-draw` v1

Traza JSONL de los comandos que el renderer decide antes de rasterizar. Es el
tercer nivel de paridad para una partida `.sav`: después de bytes
[`world-raw`](WORLD_RAW_SCHEMA.md) y de interpretación
[`world-semantic`](WORLD_SEMANTIC_SCHEMA.md), permite localizar el sprite,
paleta, orden y relación padre/hijo que causan una anomalía visual.

Se activa en el oráculo C++ con `OPENTTDRS_WORLD_DRAW_OUT`. El exportador
ejecuta los `draw_tile_proc` de OpenTTD para una región sin framebuffer,
clipping, vehículos, rótulos ni UI; por lo tanto es reproducible también con
el binario dedicado.

## Entrada y región

```text
OPENTTDRS_WORLD_DRAW_OUT=/tmp/openttd-world-draw.jsonl
OPENTTDRS_WORLD_DRAW_REGION=x0,y0,x1,y1  # inclusiva; opcional
```

Sin región se recorre el mapa entero. Para investigación se recomienda una
región pequeña alrededor del puente, túnel, estación o árbol afectado.

La primera línea es `metadata`:

```json
{
  "kind":"metadata",
  "schema_version":1,
  "contract":"world-draw",
  "producer":"openttd",
  "stage":"headless_tile_draw_proc",
  "width":256,
  "height":256,
  "region":{"min_x":120,"min_y":80,"max_x":145,"max_y":105},
  "clipping":"disabled",
  "includes":["ground","sortable","child","combine"]
}
```

Cada tesela emite una fila de contexto aun cuando no produzca sprites:

```json
{"kind":"tile","index":23947,"x":139,"y":93,"tile_type":9}
```

Una exportación válida termina con una fila de cierre. Esto distingue un
stream completo de un proceso que se interrumpió durante un `draw_tile_proc`:

```json
{"kind":"complete","tiles":676,"draws":4182}
```

Cada comando tiene un `ordinal` reiniciado por tesela:

```json
{
  "kind":"draw",
  "x":139,"y":93,"ordinal":4,
  "role":"sortable",
  "primitive":"sortable",
  "sprite":{"source":"global","id":2442,"raw_id":2442},
  "palette":0,
  "resolved_palette":0,
  "world":{"x":2224,"y":1488,"z":24},
  "bounds":{"ox":0,"oy":0,"oz":0,"ex":16,"ey":16,"ez":0},
  "offset":{"x":0,"y":0,"z":0},
  "combine_group":null,
  "parent_ordinal":null,
  "transparent":false,
  "fallback":false
}
```

Primitivas posibles:

- `ground`: llamado de suelo (`AddTileSpriteToDraw`). `offset.x/y` conserva
  sus `extra_offs_*` ya normalizados a `ZOOM_BASE`; por ejemplo, una reserva
  PBS de una vía de esquina elevada puede quedar en `y=-32` aun sin cimiento.
- `sortable`: sprite con bounding box propio.
- `combined`: hijo lógico emitido durante `StartSpriteCombine`/`EndSpriteCombine`.
- `child`: sprite de pantalla relativo a su padre/fundación.
- `empty_bounds`: separador de orden de OpenTTD (`SPR_EMPTY_BOUNDING_BOX`),
  sin píxeles propios; el comparador lo excluye de la selección visual.
- `combine_start` / `combine_end`: delimitadores explícitos de bloque.

`sprite.id` es el ID lógico global de OpenTTD, sin los modifier bits; `raw_id`
conserva esos bits. `palette` es el valor solicitado y `resolved_palette`
expone la sustitución a transparente cuando aplica. Las unidades de `world` y
`bounds` son las nativas de OpenTTD (una tesela = 16), no píxeles ni atlas de
Bevy.

El candidato puede añadir `"geometry_explicit":true` cuando ya conoce la
geometría exacta de un comando. Al usar `--geometry`, el comparador exige la
misma tupla `primitive`, `world`, `offset` y `bounds` del oráculo. Esto es
importante para `child`: un suelo dibujado después de una fundación no tiene
posición de mundo ni bounds propios (`world:null`, `bounds:null`), sino un
offset de pantalla relativo al padre. En ese caso debe conservar
`primitive:"child"`; no puede declararse como `ground` aunque el ID del sprite
sea el mismo.

La implementación candidata debe conservar la misma identidad lógica antes
de convertirla a `Handle<Image>`/índice de atlas. Las diferencias de página o
packing del atlas no pertenecen al contrato.

## Exportar y contrastar

El oráculo se extrae con `scripts/export_openttd_world_draw.sh`; el candidato
no necesita abrir una ventana ni una GPU:

```text
scripts/export_openttdrs_world_draw.sh save/Kale_TitleGame.sav /tmp/rust.jsonl 8,5,8,5
python3 scripts/compare_world_draw.py /tmp/cpp.jsonl /tmp/rust.jsonl
```

La comparación inicial es de *selección contenida*: falla si el candidato
elige un sprite inexistente en el `draw_tile_proc`, cae en fallback, o las
teselas no coinciden. También informa comandos de OpenTTD todavía sin una
familia equivalente instrumentada; esos no hacen fallar hasta completar la
cobertura de todos los spawners.

`--order` añade una comprobación de orden relativo: los comandos candidatos
deben formar una subsecuencia de los comandos visuales C++ de la misma tesela.
No compara ordinales absolutos porque el candidato todavía instrumenta sólo
algunas familias; sí exige la misma primitiva, sprite y, cuando está explícita,
paleta y geometría. Así una inversión de capas se detecta sin convertir los
comandos C++ aún no instrumentados en falsos negativos.
