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
| **RMAP-004** | Reproducir el mapa aleatorio de OpenTTD con el mismo seed desde el generador procedural Rust. | **Abierto (P1, avance parcial)** | La portabilidad de TGP/RNG, la suma de octavas de costa sin normalizar, `FixSlopes`, costas, `water_borders`, bordes `MP_VOID` y la etapa pueblos/industrias ya está en el generador. El port de `GenerateClearTile` materializa rough/rocks con su recorrido diagonal y `m5`; `GenerateObjects` ya crea transmisor/faro vanilla, escala, separación, `ObjectID` nativo y pool `OBJS`. Desde `fix(worldgen): carry OpenTTD RNG across generation phases`, el flujo `TGP → FixSlopes → GenerateClearTile → towns → industries → objects → trees` conserva un único stream y `world_raw_dumper` reproduce las 0x500 pasadas de paisaje sin adelantar la economía. TGP crudo, nivel, costas, suavizados y seno son idénticos en 65×65 puntos para la semilla `1330935378`; la prueba cubre el límite fraccionario de costa. RMAP-005 cerró el contrato local de sustrato/shore de árboles, pero no el algoritmo completo. El cargador sigue exacto 15/15; el generador mismo-seed sigue abierto porque divergen clear, población, objetos, árboles y bytes `m1..m8`. No se cierra con ajustes de renderer o del lector SAV. |
| **RMAP-005** | Contrato de sustrato y costa de `GenerateTrees`. | **Cerrado** | `CanPlantTreesOnTile` ahora rechaza fields/rocks, respeta `allow_desert`, admite sólo costa sin una única esquina elevada, usa `GetTileZ` para agrupación por altura y materializa `TreeGround::Shore` (`m1=Sea`, `m2=0xF0`) mientras limpia el estado non-flooding vecino. Tests unitarios cubren cada contrato. El oráculo 64²/seed `1330935378` vuelve a exportar y el cargador tiene 0/4096 diferencias. |
| **RMAP-006** | Diferencial completo de la fase de árboles. | **Abierto (P1, avance parcial)** | Ya se alinearon la suma `GB(Random(), 0, 5) + 25`, el divisor de fase calculado en double→float, la acumulación `float` de ángulo de `CreateRandomStarShapedPolygon` y el escalado de refuerzo por límite de altura. El fixture aislado temperate 64²/seed `1330935378` ahora es exacto; siguen pendientes nieve ártica por encima de `snow_line`, el pase de selva tropical y la integración con entradas de clear/pueblos/industrias que ya divergen en el generador completo. No se cierra por la frontera temperate sola. |
| **RMAP-007** | Oráculo de frontera pre/post `GenerateTrees`. | **Cerrado** | El hook versionado guarda ambos `.sav` y emite `tree-generation-trace`; `scripts/tree_phase_parity.py --size 64 --seed 1330935378` retoma el `DATE.random_state` real y valida el mapa por tesela/bloques 4×4 y cada `PlaceTree`. Resultado: 0/4096 teselas, 0/256 bloques y 357/357 colocaciones distintas. |
| **RMAP-008** | Estado RNG global `DATE` en interoperabilidad SAV. | **Cerrado** | `SavGame` importa `random_state[0..1]` de `DATE`, `GameState::from_sav_game` lo restaura y el escritor OTTN lo preserva. Tests cubren columnas incompletas, lectura y round-trip. El dumper usa ese estado para `--replay-trees`; no se infiere desde la semilla de creación. |
| **RMAP-009** | Límite de altura en refuerzo de árboles y PATS. | **Cerrado** | `construction.map_height_limit` se importa y reemite en `PATS`; el replay usa su valor efectivo (`0` automático → 30). `PlaceTreesRandomly` ahora escala `GetTileZ()*2` por `15 / map_height_limit`, igual que `tree_cmd.cpp`; tests cubren 15, 30 y automático, y RMAP-007 confirma el stream RNG y las 357 colocaciones exactas. La bonificación ártica de nieve sigue explícitamente en RMAP-006. |

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
