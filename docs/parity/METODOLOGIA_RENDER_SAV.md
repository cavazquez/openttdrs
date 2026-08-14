# Metodología de paridad de partidas `.sav`

Este documento deja el método y el estado del trabajo de carga y render de
partidas OpenTTD. El objetivo no es que una captura se parezca aproximadamente:
es explicar por tesela y por decisión de dibujo por qué OpenTTD y `openttdrs`
eligen un resultado distinto.

La foto del lote es el [checkpoint de pausa](../checkpoints/2026-08-09-parity-oracle-pause.md).
Es un registro histórico: `main` continuó incorporando cambios revisables
después de ese punto. El estado de una corrección se establece siempre con los
exports y comparadores de este documento, no por asumir que aquel checkpoint
sigue describiendo el árbol actual.

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
| Paleta de compañía 8bpp y estación rail vanilla | Se corrigió el desplazamiento de un índice DOS en las rampas y se dejó de inferir recolor por RGB ajeno a la rampa autora. La región `120,111..128,113` de Kale compara 118/118 comandos de estación (sprite, paleta, geometría y orden). | Validada por `world-draw`; la composición raster amplia sigue teniendo familias ajenas a esta corrección. |
| Paletas de casas vanilla | `HOUSE_DRAW_DATA` conserva ahora `p1`/`p2` de cada `M(...)` de `town_land.h`; la caché aplica paleta de compañía (775–790), estructura (795–801) e iglesia (1438–1439) a las capas de suelo y edificio. La traza registra la paleta incluso cuando la geometría no es explícita. | En Kale 8bpp, 740 draws de casa no nulos coinciden exactamente con el comando C++ del mismo sprite/tesela/paleta; las pruebas exigen que todos los pares generados tengan asset recoloreado. La captura global sigue marcada como diferente por familias ajenas. |
| Reservas PBS 8bpp | Los overlays `PALETTE_CRASH=804` se hornean desde índices DOS con la misma pseudo-sprite de recolor que usa OpenTTD; se eliminó el tinte RGBA naranja aproximado. Incluye `SINGLE_*` rail/mono/maglev y las doce rampas PBS. | Validar en captura focalizada; la traza conserva paleta 804 y ahora distingue la ausencia del asset exacto como fallback. |
| Campos y cercas de Kale (8bpp y 32bpp) | Se instrumentó `DrawTile_Clear` para campos y sus cuatro cercas, se corrigió la altura de esquina de pendientes empinadas y el suelo natural deja de usar la elevación como profundidad. El spawn recorre las teselas en el mismo barrido diagonal de `ViewportAddLandscape`; OpenGFX2 selecciona su variante RGBA normal sin mezclar coordenadas 8bpp. | La región `225,25..251,61` valida en 8bpp 647 suelos y 476 cercas: ID, geometría y orden relativo 100 % contenidos en OpenTTD. La regresión de selección cubre los dos perfiles de baseset. Falta una captura local de aceptación tras el cambio. |
| Suelo natural `MP_CLEAR` (8bpp y 32bpp) | Se porta `DrawTile_Clear`: césped por densidad, rough plano por `TileHash`, las 19 pendientes rocosas de la serie correcta y la tabla sin tintes de nieve/desierto. La traza `world-draw` registra cada suelo que no es campo. OpenGFX y OpenGFX2 no activan `SecondRockyTileSet`, por lo que ambos usan 4023–4041. | Kale completo: 9.331 selecciones `clear-ground` coinciden en sprite, geometría y orden con el comando C++. Es paridad de selección, no todavía igualdad raster global. |
| Agua plana y costa `MP_WATER` | `push_water_tile` traza `DrawSeaWater` (4061) y `DrawShoreTile` (`SPR_SHORE_BASE + tileh_to_shoresprite`), incluso cuando el cache de costa materializa la imagen desde Action5/NewGRF. Los locks y depósitos conservan sus familias separadas. | Kale completo: 8.139 `water-ground` y 1.109 `water-shore` coinciden en sprite y orden con C++; la cobertura permite que una futura regresión de costa/agua aparezca en el auditor. |
| Iconos y assets | Regeneración de iconos y datos de atlas asociados. | En checkpoint; revisión visual pendiente. |

Este inventario describe trabajo efectuado, no una afirmación de que todos los
casos estén resueltos. El checkpoint mezcla renderer, simulación, assets y
SAV; debe dividirse antes de una integración amplia.

## Evidencia y revalidación

En la partida de estrés se obtuvo inicialmente una exportación completa de
65.536 teselas con coincidencia exacta de `world-raw` y `world-semantic`. Eso
reduce mucho la probabilidad de que los artefactos tratados sean una
deserialización general del mapa, pero no sustituye volver a correr el contrato
cuando cambie el importador.

La referencia `world-draw` produjo 157.154 comandos C++ y el candidato 65.534
selecciones instrumentadas. Para las familias cubiertas —árboles, vía,
puentes, catenaria, túneles, depósito naval e hitos vanilla— las selecciones
candidatas se encontraron en OpenTTD. En la región de control `171,109..171,110`,
las siete selecciones Rust estuvieron contenidas entre diez comandos de
referencia; las tres restantes son familias aún no instrumentadas en Rust.

Esto **no** es igualdad total de sprites ni de píxeles. El comparador usa
*selección contenida*: falla si Rust elige un sprite inexistente, cae en
fallback, cambia tesela/paleta o geometría instrumentada. Los comandos C++ sin
familia Rust equivalente se informan, pero todavía no hacen fallar. Sólo cuando
todos los spawners estén cubiertos podrá usarse `--strict-reference` como gate.

### Regresión `MP_OBJECT` descubierta al revalidar

Una comparación completa posterior de `Kale_TitleGame.sav` encontró dos
divergencias raw, en `(14,141)` y `(245,240)`: OpenTTD conservaba `MAP5 = 0`
y el candidato escribía `1`. Ambas teselas eran `MP_OBJECT`; la alteración
cambiaba también el `ObjectID` y por ende el sprite de objeto elegido.

El origen era doble: se usaba `location.tile` como clave del pool `OBJS` y se
sobrescribía `MAP5` con `ObjectType`. OpenTTD hace lo contrario: forma
`ObjectID = m2 | (m5 << 16)` y consulta el tipo visual en el pool por ese ID.
La corrección conserva `MAP5`, transporta `ObjectID → ObjectType` en el footer
`OBTY` y añade `details.object_type` a `world-semantic` v2. El mismo análisis
descubrió que el conversor Python separaba `MAP2` big-endian en orden inverso;
se corrigió junto con la regresión.

El guardarraíl permanente es
`scripts/verify_parse_sav_object_m5.py`, incluido en el manifiesto de CI. Usa
un fixture con transmisor y faro, exige igualdad de `MAP5` y `ObjectID`, y
comprueba que cada objeto se resuelva desde `OBTY`. Así un arreglo visual no
puede volver a ocultar una mutación de bytes del save.

La revalidación final de `Kale_TitleGame.sav` volvió a comparar las 65.536
teselas: `world-raw` y `world-semantic` terminaron sin diferencias. La
traza `world-draw` incorpora además los hitos vanilla afectados por esta
regresión; el fixture controlado compara transmisor `(47,33)` y faro `(60,55)`
con `--strict-reference`, y en ambos casos coinciden terreno, sprite,
geometría y orden de los dos comandos de OpenTTD.

### Revalidación: campos y cercas de Kale (8bpp y 32bpp)

La región de cultivo `225,25..251,61` se comparó usando el mismo baseset
OpenGFX 8bpp para OpenTTD y para el atlas de `openttdrs`. El oráculo C++ emitió
1.957 comandos y el candidato 1.740 selecciones instrumentadas. Dentro de esa
región, los 647 `field-ground` y las 476 `field-fence` coincidieron al 100 %
en ID de sprite, geometría explícita y orden relativo. La regla de composición
no depende de la profundidad de color; para OpenGFX2, la regresión de NFO
verifica que cada campo/cerca use la continuación `32bpp` de zoom normal y no
la fila 8bpp ni una variante `zi4`.

El defecto visual que quedaba no era de importación ni de selección: el
renderer Rust sumaba `height * 0.001` a la profundidad de todo suelo. OpenTTD
inserta `DrawGroundSprite` en un pase separado, barrido por `x + y` y luego
`y - x`; la altura sólo cambia la posición en pantalla. Ahora el suelo natural
usa esa profundidad diagonal, y `TileViewportBounds::iter_coords` reproduce el
barrido C++ para conservar el desempate incluso si dos valores `f32` coinciden.
Las rampas/fundaciones conservan su orden local especial y no forman parte de
este cambio.

La estación ferroviaria vanilla en `(226,42)` y `(227,42)` quedó explicada y
cubierta por regresión. La tesela tiene `RailType=Monorail`; OpenTTD aplica
`RailTypeInfo::GetRailtypeSpriteOffset()` (+82) tanto al suelo como a cada
capa de `DrawRailTileSeq`. Por eso emite `1093/1159/1151/1162/1166`, no la
familia rail/elrail `1011/1077/1069/1080/1084`. El cliente replica ese
contrato para rail/elrail, monorail y maglev (+0/+82/+164), recorta las tres
familias desde el NFO del perfil gráfico activo y conserva los offsets NFO
propios de 8bpp o 32bpp. La caja `TILE_SEQ` de los pilares Y-rear (1077 y sus
variantes) también quedó corregida a `5×16×2`, según `station_land.h`.

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

### Nivel 4: raster focalizado

Cuando raw, semántica y draw explican las decisiones pero la composición aún
parece distinta, capturar ambos viewports exactamente en la misma tesela. El
orquestador fija zoom normal/escala 1, pausa ambos motores, oculta UI/rótulos/
vehículos y usa el mismo perfil OpenGFX 8bpp; deja referencia, candidata, diff
y reporte hashable en un directorio. Para investigar una capa dinámica se
puede desactivar sólo esa limpieza con `OPENTTDRS_WORLD_SCREENSHOT_CLEAN=0`.

```bash
./scripts/compare_focused_world_screenshot.sh \
  "$SAV" /tmp/kale-raster-189-126 189,126 1280x720 1
```

El reporte incluye un registro de cámara de hasta ocho píxeles. Un
desplazamiento distinto de cero es una señal a investigar, no una corrección
que permita dar por buena la paridad. Ver el
[contrato raster](WORLD_SCREENSHOT_SCHEMA.md) para la semántica completa.

Para el caso de regresión de la estación vanilla de Kale:

```bash
REGION=120,111,128,113
./scripts/export_openttd_world_draw.sh "$SAV" /tmp/kale-ottd-station.jsonl "$OTTD_BIN" "$REGION"
./scripts/export_openttdrs_world_draw.sh "$SAV" /tmp/kale-rust-station.jsonl "$REGION"
python3 scripts/compare_world_draw.py /tmp/kale-ottd-station.jsonl /tmp/kale-rust-station.jsonl \
  --geometry --order --by-role
```

Para repetir la regresión de campos/cercas en 8bpp:

```bash
REGION=225,25,251,61
./scripts/export_openttd_world_draw.sh "$SAV" /tmp/kale-ottd-fields.jsonl "$OTTD_BIN" "$REGION"
RUSTC_WRAPPER='' ./scripts/export_openttdrs_world_draw.sh "$SAV" /tmp/kale-rust-fields.jsonl "$REGION"
python3 scripts/compare_world_draw.py /tmp/kale-ottd-fields.jsonl /tmp/kale-rust-fields.jsonl \
  --geometry --order --by-role
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
