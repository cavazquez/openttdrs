# Metodología de paridad de partidas `.sav`

Este documento deja el método y el estado del trabajo de carga y render de
partidas OpenTTD. El objetivo no es que una captura se parezca aproximadamente:
es explicar por tesela y por decisión de dibujo por qué OpenTTD y `openttdrs`
eligen un resultado distinto.

La foto del lote es el [checkpoint de pausa](../checkpoints/2026-08-09-parity-oracle-pause.md).
El `main` estable quedó en `5b0023b`; el trabajo experimental está en
`checkpoint/pause-2026-08-09-parity-oracle` hasta que se separe en cambios
revisables y vuelva a pasar toda la suite.

## Alcance y repositorios

| Componente | Uso | Regla de trabajo |
|---|---|---|
| `openttdrs` | Implementación Rust, importador SAV, renderer y comparadores | Trabajar primero en un checkpoint; integrar a `main` sólo cambios acotados y verdes. |
| Fork `cavazquez/OpenTTD` | Referencia C++ ejecutable | `main` sigue OpenTTD oficial. Las sondas viven únicamente en `openttdrs/oracle-parity`, nunca en upstream. |
| Partida `.sav` fija | Caso reproducible | Ambas exportaciones usan el mismo archivo y SHA-256; `Kale_TitleGame.sav` se utilizó como caso de estrés. |

El fork C++ no cambia el resultado que se estudia: agrega exportadores de
diagnóstico. Así se puede saber qué bytes hay en una coordenada, cómo se
interpretan y qué comandos produce el `draw_tile_proc` real de OpenTTD.

## Modelo de evidencia

Una captura puede fallar por bytes mal cargados, semántica mal interpretada,
sprite equivocado, geometría/altura incorrecta o por orden de dibujo. Por eso
la paridad se comprueba por capas, de menor a mayor distancia del píxel final.

| Nivel | Contrato | Pregunta | Límite |
|---|---|---|---|
| 1 | [`world-raw`](WORLD_RAW_SCHEMA.md) | ¿Los bytes de mapa de cada tesela son los mismos? | No explica su significado. |
| 2 | [`world-semantic`](WORLD_SEMANTIC_SCHEMA.md) | ¿Ambos clasifican igual vía, puente, túnel, estación, pendiente y orientación? | No garantiza el sprite final. |
| 3 | [`world-draw`](WORLD_DRAW_SCHEMA.md) | ¿Rust selecciona sprite, paleta y geometría permitidos por el `draw_tile_proc` C++? | La cobertura Rust aún no incluye todas las familias. |
| 4 | Captura enfocada | ¿La composición completa se ve correcta en el contexto real? | Es aceptación visual, no la única evidencia. |

La regla es encontrar la primera capa que diverge antes de editar. De ese modo
no se oculta un error de importación con un ajuste visual, ni se culpa al atlas
cuando el tile ya se decodificó mal.

## Trabajo incorporado al checkpoint

### Infraestructura de diagnóstico

- Exportadores C++ de `world-raw`, `world-semantic`, `world-draw` y captura
  del mundo. `world-draw` ejecuta `draw_tile_proc` sin framebuffer, UI,
  vehículos ni rótulos y puede correr headless.
- Dumper Rust equivalente para raw y semántica, más una traza headless de las
  decisiones visuales que el renderer ya instrumenta.
- Comparadores JSONL que validan encabezado, región, hash de la partida,
  teselas, sprites, paletas, fundaciones y geometría cuando está disponible.
  Las pruebas de mutación comprueban que detecten diferencias deliberadas.
- Scripts que preparan el baseset, ejecutan ambos lados y exigen una fila final
  `complete`; una traza truncada no es evidencia válida.
- `sccache` en desarrollo y GitHub Actions para acelerar las iteraciones de
  compilación y pruebas.

### Familias visuales tratadas

| Familia | Trabajo realizado | Estado |
|---|---|---|
| Viewport de mapas grandes | Completado de chunks parciales. | Corrección aislada publicada en `main` (`5b0023b`). |
| Terreno, pendientes y árboles | Revisión de altura isométrica y sprites de ladera para eliminar artefactos que parecían vías o dejaban colores extraños. | En checkpoint; requiere captura focalizada. |
| Puentes rail y conexiones | Fundaciones, pilares, rampas y altura efectiva; la traza conserva sprites y geometría de las transiciones. | En checkpoint; sin declarar paridad total. |
| Catenaria | Wire/pylon y altura efectiva de puente/túnel para que el tendido no flote ni se corte. | Instrumentada; faltan más regresiones. |
| Monorriel y maglev | Selección diagonal tipada por railtype para no usar rail convencional. | En checkpoint; validar por región. |
| Depósitos y estaciones especiales | Geometría de depósito naval, reserva visual de depósito rail y distinción de tiles especiales para no disfrazar un fallback como parada de buses. | En checkpoint; los fallbacks deben ser explícitos. |
| Iconos y assets | Regeneración de iconos y datos de atlas asociados. | En checkpoint; revisión visual pendiente. |

Este inventario describe trabajo efectuado, no una afirmación de que todos los
casos estén resueltos. El checkpoint mezcla renderer, simulación, assets y
SAV; debe dividirse antes de una integración amplia.

## Evidencia obtenida hasta la pausa

En la partida de estrés se obtuvo una exportación completa de 65.536 teselas
con coincidencia exacta de `world-raw` y `world-semantic`. Eso reduce mucho la
probabilidad de que los artefactos tratados sean una deserialización general
del mapa.

La referencia `world-draw` produjo 157.154 comandos C++ y el candidato 65.534
selecciones instrumentadas. Para las familias cubiertas —árboles, vía,
puentes, catenaria, túneles y depósito naval— las selecciones candidatas se
encontraron en OpenTTD. En la región de control `171,109..171,110`, las siete
selecciones Rust estuvieron contenidas entre diez comandos de referencia; las
tres restantes son familias aún no instrumentadas en Rust.

Esto **no** es igualdad total de sprites ni de píxeles. El comparador usa
*selección contenida*: falla si Rust elige un sprite inexistente, cae en
fallback, cambia tesela/paleta o geometría instrumentada. Los comandos C++ sin
familia Rust equivalente se informan, pero todavía no hacen fallar. Sólo cuando
todos los spawners estén cubiertos podrá usarse `--strict-reference` como gate.

## Procedimiento para investigar un caso nuevo

1. Reproducirlo con una partida inmutable y elegir una región mínima: los dos
   extremos de un puente, la boca de un túnel o el depósito afectado.
2. Comparar raw y semántica. Si alguno falla, arreglar importación o
   decodificación antes de tocar renderer.
3. Exportar `world-draw` de ambos lados para la región. Comparar primero
   sprite, rol, paleta, fundación y geometría.
4. Seguir en C++ el `draw_tile_proc` y sus auxiliares; contrastar en Rust los
   bytes del tile, vecinos, pendiente, eje, railtype, reserva y altura efectiva.
5. Aplicar el cambio mínimo en la capa responsable, agregar una prueba/fixture
   del caso y repetir la traza. Recién después revisar la captura amplia.
6. Extraer el arreglo del checkpoint en un commit temático y verde. No mezclar
   un arreglo visual con simulación, atlas o SAV salvo que la traza pruebe la
   dependencia.

### Comandos reproducibles

Los scripts reciben una región inclusiva `x0,y0,x1,y1`. Ajustar las rutas al
entorno local. El binario C++ debe venir de la rama `openttdrs/oracle-parity`
del fork, no de su `main` oficial.

```bash
SAV=save/Kale_TitleGame.sav
OTTD_BIN=/ruta/a/OpenTTD/build/openttd
REGION=171,109,171,110

# Nivel 1: bytes de mapa.
./scripts/export_openttd_world_raw.sh "$SAV" /tmp/openttd-raw.jsonl "$OTTD_BIN" "$REGION"
./scripts/export_openttdrs_world_raw.sh "$SAV" /tmp/openttdrs-raw.jsonl "$REGION"
python3 scripts/compare_world_raw.py /tmp/openttd-raw.jsonl /tmp/openttdrs-raw.jsonl --strict-metadata

# Nivel 2: semántica decodificada.
./scripts/export_openttd_world_semantic.sh "$SAV" /tmp/openttd-semantic.jsonl "$OTTD_BIN" "$REGION"
./scripts/export_openttdrs_world_semantic.sh "$SAV" /tmp/openttdrs-semantic.jsonl "$REGION"
python3 scripts/compare_world_semantic.py /tmp/openttd-semantic.jsonl /tmp/openttdrs-semantic.jsonl \
  --strict-metadata --show-inventory

# Nivel 3: decisiones de dibujo.
OPENTTDRS_OPENGFX_DIR=/ruta/a/opengfx \
  ./scripts/export_openttd_world_draw.sh "$SAV" /tmp/openttd-draw.jsonl "$OTTD_BIN" "$REGION"
./scripts/export_openttdrs_world_draw.sh "$SAV" /tmp/openttdrs-draw.jsonl "$REGION"
python3 scripts/compare_world_draw.py /tmp/openttd-draw.jsonl /tmp/openttdrs-draw.jsonl \
  --geometry --foundations --order --by-role
```

Cuando se comparte cache de compilación, ejecutar con
`CARGO_INCREMENTAL=0`, `CARGO_NET_OFFLINE=true` y `RUSTC_WRAPPER=sccache`.
La configuración de compilación no debe cambiar la partida ni su traza.

## Interpretación de resultados

- **Falla raw:** revisar lector SAV, orden de teselas, versión del chunk o
  bytes `m1..m8` antes de mirar sprites.
- **Raw coincide y falla semántica:** revisar trackbits, railtype, eje,
  pendiente, extremos de túnel/puente, estación o depot.
- **Semántica coincide y falla draw:** revisar selección de sprite, paleta,
  fundación, offset y altura; vecinos y PBS pueden importar.
- **Orden relativo falla:** el candidato eligió comandos válidos, pero alteró
  la composición de capas de OpenTTD; inspeccionar la tesela y los vecinos que
  informa `candidate_order_missing_in_reference`.
- **Draw contenido pasa pero la captura falla:** puede faltar instrumentación,
  orden/solapamiento, clipping/cámara, atlas o una primitiva fuera de traza.
  Extender la traza antes de adivinar.
- **Hay fallback:** tratarlo como fallo explícito, con rol y motivo visibles;
  nunca reutilizar un sprite que parezca un objeto válido de otra clase.

## Límites y próximo hito

La cobertura `world-draw` es intencionalmente parcial. El siguiente hito es
instrumentar los spawners genéricos de suelo, casas, estaciones, carreteras y
otros objetos hasta poder exigir `--strict-reference` por región.

Antes de extraer cambios del checkpoint deben resolverse también los fallos de
`cargo test --workspace --no-fail-fast`:

1. `newgrf_actions::tests::truncated_badge_list_emits_diagnostics_and_inspect_warning`.
2. `pbs_dual_curve_oracle::rust_matches_openttd_oracle_for_forty_ticks`.
3. `sav_load_stationlist::stationlist_depot_row_connects_to_rail`.

El estado exacto y la validación de la pausa se mantienen en el
[checkpoint](../checkpoints/2026-08-09-parity-oracle-pause.md). Los esquemas
son los contratos de largo plazo; este documento es el método operativo para
aplicarlos.
