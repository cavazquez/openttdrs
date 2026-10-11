# Conexiones de puentes de Kale

Actualizado: **2026-10-10**. Estado: **aceptado en este corte**. Caso abierto
por la captura del usuario de los puentes y terraplenes de Rindington.

La fuente es `save/Kale_TitleGame.sav`, SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`.
El oráculo de geometría y raster es OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`, con OpenGFX 8.0 y blitter
`8bpp-simple`. La revisión usa la región `(185,95)..(217,146)` y la captura
de 800×600 centrada en `(204,117)`, una tesela plana. No desplaza ni alinea
los PNG después de capturarlos.

## Por qué escapó al control anterior

El gate anterior medía los píxeles que **empeoraban frente al cliente
anterior**, comparando ambas versiones con OpenTTD desde el encuadre fijo
`(132,2)`. La aceptación de menos de 1.000 píxeles empeorados no exigía menos
de 1.000 diferencias absolutas con OpenTTD. Un faltante presente en ambas
versiones no consumía ese presupuesto. Ese encuadre tampoco sustituye una
revisión localizada de los puentes que señaló el usuario.

Las trazas previas conservaban el ID, las cajas y las relaciones de los
sprites seleccionados, pero no comprobaban su posición final y su orden
después del recorte por bandas. Aquí el renderer seleccionaba las piezas
correctas y aun así ocultaba la vía. Declarar correcta la escena a partir de
la aceptación incremental y de esas trazas fue una conclusión demasiado amplia.

## Divergencias corregidas

1. El renderer restaba `BRIDGE_Z_START` a cabeceras y tablero. En
   `tunnelbridge_cmd.cpp`, `DrawTile_TunnelBridge` coloca las cabeceras en
   `ti->z` después de la fundación; sólo `DrawBridgeMiddle` resta esos tres
   píxeles. El desplazamiento compartido bajaba todas las bocas y rompía la
   unión con la vía o carretera adyacente. Ahora se conserva la regla propia
   de cada pieza, también en la rama de cabeceras custom.
2. Las ventanas de profundidad de los children terminaban en el siguiente
   parent global. Una cabecera representada por un proxy de banda local
   quedaba fuera de ese límite. Su vía y el pasto del cimiento podían recibir
   exactamente el mismo Z; Bevy desempata esas transparencias por la entidad
   y el pasto podía tapar la vía. Los proxies locales ahora limitan también
   las ventanas de los children, conservando el bloque nativo
   **cimiento → suelo → cabecera**. Se mantiene el mismo límite para la fuente
   invisible de un child promovido.
3. Durante la captura con atlas de zoom nativo, el redondeo ocurría antes del
   sorter y sólo al agregar el sprite. El siguiente recorte restauraba una
   posición fraccionaria. Ahora se redondea después del recorte y se repite
   cuando cambia la pose de una fuente o proxy segmentado. Para esas piezas
   se aplica también en Out8x el redondeo hacia arriba de `GfxBlitter` /
   `UnScaleByZoom`. En Out4x y Out8x con cámara limitada por el borde se
   recupera primero la cámara sin su compensación de muestreo, se enmascara
   la esquina a bloques enteros de zoom como `ViewportDoDraw`, y se calcula
   el `ceil` respecto de ese origen. X e Y pueden tener fases distintas.
   Esta tercera corrección está limitada al modo de captura existente;
   las dos primeras corrigen el renderer del juego.

El caso decisivo es la tesela `(204,112)`: el oráculo emite el cimiento
**5423**, su suelo child **3981** y la cabecera maglev **4372**. En la captura
fallida el pasto y la cabecera acababan ambos en Z `3.1613595`. Por ejemplo,
el píxel `(237,205)` era pasto RGBA `(60,88,48,255)`, donde OpenTTD dibujaba
vía `(148,149,148,255)`. La corrección del orden recupera ese píxel y otros
dos testigos de la segunda cabecera. Las vías normales adyacentes ya estaban
instanciadas: el faltante visible provenía de la superposición de las bocas.

## Regresiones y reproducción

`bridge_heads_use_native_draw_origin_in_all_four_directions` comprueba las
posiciones reales de spawn en carretera y ferrocarril, ambos ejes y sus dos
extremos, con suelo plano y fundación nivelada. El tablero mantiene su offset
nativo de tres píxeles.

`kale_foundation_ground_stays_before_segmented_bridge_head` reproduce las
cajas y dimensiones de la tesela anterior, ejecuta el sorter y el sync de
children durante varias pasadas y exige un orden estricto de profundidad.
La reproducción inicial falló con el código anterior por el empate entre
suelo y cabecera; el escenario aceptado usa el mismo viewport 800×600 que
las capturas. `native_segmented_position_rounding_survives_recropping` usa
el ancla NFO y la cámara de esa cabecera en Out4x y Out8x, y restaura su pose
sin redondear durante tres actualizaciones para comprobar que se corrige
cada vez.
También reproduce la cabecera **2440** en `(108,26)` con las cámaras reales
del borde: `ViewportDoDraw` usa los orígenes `(-4308,-1676)` en Out4x y
`(-3912,-1472)` en Out8x. El blitter la coloca en `(416,116)` y `(159,84)`
respectivamente. La regresión exige esas poses después de cada recorte.

La prueba diferencial ignorada `kale_bridge_draw_origins_match_native_oracle`
compara la altura de origen de **30 cabeceras y 45 suelos de tablero** con
draw calls nativos de la región, emparejados por tesela, ID y clave de
inserción. Las 75 alturas coinciden. Se repitió también con el stream de
todo el mapa: coinciden **428 cabeceras ferroviarias y de carretera y 647
suelos de tablero**. Exige un stream completo del oráculo y escribe el
resultado falso antes de preparar la fixture. Se ejecuta así desde la raíz
del repositorio; los paths de entorno deben ser absolutos porque Cargo
ejecuta el test desde la carpeta del crate:

```sh
mkdir -p artifacts/transport-follow/bridges
OPENTTDRS_OPENTTD_COMMIT=14ec60f248547d4d062a1160f0fc26d742319888 \
  scripts/export_openttd_world_draw.sh save/Kale_TitleGame.sav \
  artifacts/transport-follow/bridges/native-draw.jsonl \
  reference/openttd-15.3-oracle/build/openttd 185,95,217,146
OPENTTDRS_BRIDGE_SAV="$PWD/save/Kale_TitleGame.sav" \
OPENTTDRS_BRIDGE_ORACLE="$PWD/artifacts/transport-follow/bridges/native-draw.jsonl" \
OPENTTDRS_BRIDGE_ORIGINS_OUT="$PWD/artifacts/transport-follow/bridges/origins.json" \
  cargo test -p openttdrs-client --bin openttdrs-client \
  kale_bridge_draw_origins_match_native_oracle -- --ignored --nocapture
cargo build --release --locked -p openttdrs-client
python3 scripts/capture_kale_bridge_heads.py \
  --binary target/release/openttdrs-client \
  --out artifacts/transport-follow/bridges/new-capture
```

El capturador conserva el SAV, hash del binario, perfil nuevo, pausa, centro,
resolución y 40 frames de estabilización. Exige código 0 y PNG 800×600 en
cada escala; no acepta una imagen que exista si el proceso terminó mal.
`--software` usa Xvfb/Lavapipe; `--trace --scales 1` registra además las
poses y texturas reales. El resultado empieza falso y sólo pasa al completar
todas las escalas solicitadas.

## Conectividad y trenes

```sh
cargo run --release --locked -p openttdrs-core \
  --example kale_bridge_crossings -- save/Kale_TitleGame.sav \
  artifacts/transport-follow/bridges/crossings.csv
```

La prueba carga la **flota y órdenes originales**, sin construir ni mover
vehículos artificialmente. Desactiva IA, desastres y averías. Encuentra
**106 pares de puentes ferroviarios**, todos con búsqueda simétrica del
otro extremo y sin cabeceras huérfanas. En 5.000 ticks observa **seis cruces**,
uno regional: el tren **1282**, tick **683**, del puente TubularSteel entre
`(203,98)` y `(209,98)`. Son diez los pares ferroviarios de la región revisada.

La historia de teselas permite detectar también un tick rápido que termina
después de la salida. La prueba falla si encuentra un extremo huérfano o no
observa ningún cruce regional. El testigo demuestra que un tren de Kale usa
un puente existente de la zona; no afirma que haya cruzado cada puente de
la fotografía. Tampoco certifica duración, altura interpolada ni física
completa del recorrido de un puente frente al motor nativo.

## Resultado y límites

La matriz localizada compara las seis imágenes completas, sin alineación,
con el cliente publicado `273e894a` y OpenTTD 15.3. El binario corregido es
SHA-256 `941c3a6d17c7be59f45efe514213997dda46b74bdeb5d842922b2c1626ad6c96`.
En orden de escala de cámara, los píxeles que empeoran / mejoran son:

- `0.25`: **16 / 39.040**.
- `0.5`: **4 / 18.100**.
- `1`: **2 / 7.518**.
- `2`: **953 / 2.563**.
- `4`: **242 / 8.361**.
- `8`: **827 / 7.268**.

Las seis cumplen el presupuesto acordado de **menos de 1.000 píxeles que
empeoren por imagen de 480.000 píxeles**. Los tres píxeles testigo de las
cabeceras coinciden con OpenTTD en la captura final. El criterio adicional
de no perder ningún píxel correcto falla en `2`, `4` y `8` y se conserva
como tal; no se sustituye por el presupuesto aceptado.

Las diferencias absolutas finales con OpenTTD son **15.608, 17.746,
12.449, 297.573, 40.948 y 307.156** en esas escalas. Son parte de la
evidencia, no una afirmación de paridad global. La revisión localizada y
los tests del bloque cimiento/suelo/cabecera evitan que un faltante ya
presente quede explicado solamente por una cifra incremental aceptable.

La matriz del encuadre histórico `(132,2)`, con 180 frames de estabilización,
también cumple el presupuesto en sus seis imágenes: **0, 0, 0, 13, 199 y
339** píxeles empeorados. Mejora **0, 0, 0, 653, 2.754 y 1.077**. Conserva
el oráculo histórico de commit `c2661164` utilizado en la aceptación de
Bevy y del camión. La nueva corrida diagnóstica de 15.3 se excluye de la
aceptación: en Out2x su cámara todavía se movía, aunque los otros cinco PNG
decodificados coinciden con el oráculo histórico. No se alinea una cámara
inestable para aceptar un resultado.

Pasan **3.086 tests del core y 1.721 del cliente**, las dos ejecuciones
adicionales del oráculo de alturas, Clippy de todos los targets con
`-D warnings`, formato, compilación release, sintaxis del capturador y
frescura de la documentación. Se conservan seis tests ignorados del core y
tres del cliente; el de alturas se ejecutó explícitamente para ambas regiones.

El [recibo de evidencia](evidence/kale-bridge-connections-20261010.json)
conserva hashes, métricas completas, streams de referencia, logs y los
intentos rechazados. Sus capturas y trazas permanecen en
`artifacts/transport-follow/bridges`. No se declara paridad global del
renderer, de la flota ni de geometrías NewGRF por esta fixture. Los FPS
siguen aplazados. Este caso concluye el alcance solicitado; no abre trabajo
autónomo sobre el resto del backlog.
