# Paridad de mapas aleatorios

Esta matriz separa dos contratos que suelen confundirse:

1. **Interoperabilidad `.sav`**: OpenTTD genera una partida aleatoria y
   `openttdrs` abre exactamente esa partida.
2. **Generación con el mismo seed**: ambos generadores crean un mapa nuevo
   desde cero con las mismas dimensiones y semilla.

El primer contrato es el que permite abrir mapas de OpenTTD. El segundo exige
portar el algoritmo de `genworld` y no se deduce de que el cargador sea
compatible.

## Método reproducible

La herramienta [`scripts/random_map_parity.py`](../../scripts/random_map_parity.py)
usa OpenTTD headless para generar el mapa y un `.sav` por caso. El hook de
exportación escribe el contrato [`world-raw`](WORLD_RAW_SCHEMA.md) y conserva
el `.sav`; luego `world_raw_dumper` carga el `.sav` real y vuelve a exportar sus
teselas desde Rust. La comparación primaria es exacta sobre `height`, `type` y
`m1..m8`, en orden fila-mayor. Para localizar divergencias se agrupan las
teselas en bloques de 4×4; no se usa una captura de pantalla grande como
criterio de igualdad.

El candidato administrado se verifica con `cargo build --locked` antes de
cada corrida, aunque `world_raw_dumper` ya exista. Un error de compilación
aborta la comparación. Los reportes registran SHA-256 del ejecutable, commit
y cambios locales de archivos versionados; el commit solo no identifica un
árbol modificado. `--candidate-bin` permite un ejecutable externo explícito:
se registra su hash sin atribuirlo al código de este checkout. Esta protección
también se aplica al comparador por fases (RMAP-142).

La corrida completa usa muchas semillas en el tamaño mínimo y reduce la
cantidad al crecer:

```text
64×64:   8 mapas
128×128: 4 mapas
256×256: 2 mapas
512×512: 1 mapa
```

Ejemplo, incluyendo el contraste del generador Rust:

```bash
python3 scripts/random_map_parity.py \
  --reference-commit "$(git -C reference/openttd-upstream rev-parse HEAD)" \
  --compare-generator \
  --out-dir /tmp/openttdrs-random-map-matrix \
  --report /tmp/openttdrs-random-map-matrix/report.json \
  --keep-artifacts
```

Sin `--compare-generator` la herramienta sólo valida la apertura del `.sav` y
termina con código cero cuando no hay diferencias. Con esa opción el código de
salida es no cero si el generador procedural Rust no reproduce el mapa de
OpenTTD; eso es una señal de brecha de producto, no un fallo del cargador.
Para que la comparación use el mismo punto de observación que el hook de
OpenTTD, el harness ejecuta `OPENTTDRS_GENERATE_STARTUP_TICKS=1280` en el
generador Rust: `genworld.cpp` corre `0x500` ciclos de `RunTileLoop` y el
primer `StateGameLoop` ejecuta `AnimateAnimatedTiles`, `RunTileLoop` con el
contador `0x501` y `OnTick_Trees` antes de entregar la partida nueva.

## Resultado observado (15 mapas)

La evidencia compacta está en
[`evidence/random-map-matrix/report.json`](evidence/random-map-matrix/report.json).
El checkout de referencia usado para esta corrida coincide con el pin canónico
documentado de OpenTTD 15.3: `14ec60f248547d4d062a1160f0fc26d742319888`.
La evidencia se considera válida para esta cohorte; las configuraciones y
fases que quedan fuera de ella deben medirse antes de convertirse en gate de
release.

| Tamaño | Casos | Teselas por caso | Bloques 4×4 por caso | Apertura `.sav` | Generador mismo seed |
|---:|---:|---:|---:|:---:|:---:|
| 64×64 | 8 | 4.096 | 256 | **8/8 exactos** | **8/8 exactos** |
| 128×128 | 4 | 16.384 | 1.024 | **4/4 exactos** | **4/4 exactos** |
| 256×256 | 2 | 65.536 | 4.096 | **2/2 exactos** | **2/2 exactos** |
| 512×512 | 1 | 262.144 | 16.384 | **1/1 exacto** | **1/1 exacto** |
| **Total** | **15** | — | — | **15/15; 0 teselas y 0 bloques cambiados** | **15/15; 0 teselas y 0 bloques cambiados** |

La cohorte reproducible queda exacta después de observar el mismo límite de
`GenerateWorld` que el hook nativo: en las 15 semillas no hay diferencias de
teselas ni de bloques 4×4, tanto al cargar el `.sav` como al generar con el
mismo seed. El cambio clave fue completar la primera transición de entrega:
animación de industrias, el siguiente `RunTileLoop` y `OnTick_Trees`, incluyendo
el `RandomTileSeed` y los bytes crudos de un árbol. Esto cierra únicamente esa
frontera temporal; no implica que `GenerateLandscape`/`genworld` sea equivalente
para tamaños, climas, settings de ríos u otros ticks fuera de la matriz.

La corrida usa el pin canónico de OpenTTD 15.3 (`14ec60f248547d4d062a1160f0fc26d742319888`)
y queda registrada como evidencia de la cohorte exacta, no como cierre de
RMAP-004. Las configuraciones y fases que no están en la matriz siguen siendo
trabajo pendiente.

## Estado e issues

El registro de trabajo, con criterios de cierre y estado actual, está en
[`random-map-issues.md`](random-map-issues.md). La infraestructura de
generación, exportación, carga y comparación ya está completada y esta cohorte
queda exacta. RMAP-004 sigue abierto como gap de cobertura del producto:
equivalencia del generador en tamaños, climas, settings y fases aún no medidas;
no se marca como resuelto por una sola matriz exacta.
