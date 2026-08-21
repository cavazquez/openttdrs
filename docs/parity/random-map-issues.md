# Issues de la matriz de mapas aleatorios

Este registro conserva issues reproducibles aun cuando la sesión local no
tenga un token válido para abrirlos automáticamente en GitHub. Cada issue
tiene un criterio verificable y apunta a la evidencia; no se declaran como
"paridad" las pruebas que sólo demuestran que el cargador acepta un `.sav`.

| ID | Tema | Estado | Criterio de cierre / evidencia |
|---|---|:---:|---|
| **RMAP-001** | Generar una matriz progresiva de mapas aleatorios (64² → 512²) con semillas deterministas y artefactos aislados. | **Cerrado** | `scripts/random_map_parity.py`, matriz 64:8/128:4/256:2/512:1 y tests en `scripts/test_random_map_parity.py`. La corrida contiene 15 casos sin errores. |
| **RMAP-002** | Comparar el mapa generado por OpenTTD con el mapa que abre `openttdrs`, tesela por tesela y por bloques 4×4. | **Cerrado** | Hook `OPENTTDRS_RANDOM_MAP_*` + `world_raw_dumper` sobre el `.sav` real: 15/15 exactos, 0 teselas distintas y 0 bloques 4×4 distintos. |
| **RMAP-003** | Evitar que una imagen raster enorme o el escalado oculte divergencias del mapa. | **Cerrado** | La comparación primaria no rasteriza: valida 10 campos por tesela, cuenta diferencias y localiza bloques 4×4. La captura queda sólo como diagnóstico secundario. |
| **RMAP-004** | Reproducir el mapa aleatorio de OpenTTD con el mismo seed desde el generador procedural Rust. | **Abierto (P1)** | En la matriz actual 0/15 casos son exactos; 15/15 cambian 100% de teselas y 100% de bloques 4×4. Requiere portar/alinear el algoritmo de `genworld`, parámetros, RNG, costas, alturas, agua y campos `m1..m8`; no se puede cerrar con ajustes de renderer o del lector SAV. |

## Nota sobre issues remotos

Los IDs anteriores son estables dentro del repositorio y están listos para
copiarse a GitHub. La creación automática de issues remotos requiere renovar
la sesión de `gh` (`gh auth status` informa que el token actual es inválido);
no se inventaron números de issue ni se reutilizaron los de composición visual.
