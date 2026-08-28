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
| **RMAP-004** | Reproducir el mapa aleatorio de OpenTTD con el mismo seed desde el generador procedural Rust. | **Abierto (P1, avance parcial)** | La portabilidad de TGP/RNG, la suma de octavas de costa sin normalizar, `FixSlopes`, costas, `water_borders`, bordes `MP_VOID` y la etapa pueblos/industrias ya está en el generador. El port de `GenerateClearTile` materializa rough/rocks con su recorrido diagonal y `m5`; el port de `GenerateTrees` usa el `Randomizer` de OpenTTD, grupos, `PlaceTree`, extras por altura y el layout `m1..m5`, y se ejecuta después de objetos/industrias; sus pruebas de contrato pasan. `GenerateObjects` ya crea transmisor y faro vanilla con la secuencia de intentos, escala por tamaño/agua, separación, `ObjectID` nativo (`m2/m5`), random por tesela y pool `OBJS`; la resolución visual usa ese pool también en mapas generados. Desde el corte `fix(worldgen): carry OpenTTD RNG across generation phases`, el flujo `TGP → FixSlopes → GenerateClearTile → towns → industries → objects → trees` conserva un único stream, también al generar desde la herramienta de matriz y desde Nueva partida. La herramienta `world_raw_dumper` ya ejecuta una ruta separada de `RunTileLoop` durante `OPENTTDRS_GENERATE_STARTUP_TICKS`: actualiza el LFSR y despacha sólo callbacks de paisaje, sin adelantar calendario, vehículos ni economía; el RNG de generación se transfiere al estado antes de esas pasadas. El bloque de `TileLoop_Water` ahora descarta `MP_VOID` como hace `IsValidTile`, corrigiendo el bit `m3` de agua en bordes. La comparación por etapas con el oráculo confirmó que TGP crudo, ajuste de nivel, costas, suavizado de pendientes, suavizado de costas y transformación seno son ahora idénticos en 65×65 puntos para la semilla `1330935378`; la prueba de regresión cubre el límite fraccionario de costa. En la corrida local posterior de 15 casos, el cargador sigue exacto 15/15; el generador mismo-seed continúa 0/15 exacto porque todavía divergen las fases posteriores de clear, pueblos, industrias, objetos, árboles y los bytes `m1..m8`. Siguen incompletos el consumo/algoritmo exacto de esas fases, zonas tropicales y los contratos de entidades. No se cierra con ajustes de renderer o del lector SAV. |

El avance de código de RMAP-004 deja un contrato reproducible para aislar
terreno: `OPENTTDRS_GENERATE_POPULATION=0` omite pueblos/industrias,
`OPENTTDRS_GENERATE_STARTUP_TICKS=N` reproduce los ciclos de tile loop y
`OPENTTDRS_WATER_BORDERS` permite fijar la máscara de costas. La comparación
completa continúa usando la configuración de partida nueva (población
activada por defecto), para no confundir un mapa jugable con un heightmap.

## Nota sobre issues remotos

Los IDs anteriores son estables dentro del repositorio y están listos para
copiarse a GitHub. La creación automática de issues remotos requiere renovar
la sesión de `gh` (`gh auth status` informa que el token actual es inválido);
no se inventaron números de issue ni se reutilizaron los de composición visual.
