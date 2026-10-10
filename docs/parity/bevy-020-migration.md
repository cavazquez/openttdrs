# Migración a Bevy 0.20

Actualizado: **2026-10-10**. Corte acotado del cliente; no certifica paridad
global, 30 FPS ni una nueva publicación alpha/Snap.

## Criterio acordado

El usuario aceptó variaciones menores documentadas y fijó **menos de 1.000
píxeles que empeoren por imagen** en los seis zooms. Las mejoras se aceptan.
La comparación conserva el frame RGBA completo de 800×600, sin alineación,
recortes, máscaras dinámicas ni compensación entre mejoras y pérdidas.

Un píxel empeora si aumenta la suma de sus deltas absolutos RGBA frente al
oracle OpenTTD, comparando Bevy 0.19 y 0.20 en la misma coordenada. También
cuenta un píxel que ya era incorrecto y empeora más. El informe registra por
separado cambios totales, mejoras, coincidencias ganadas/perdidas y bloques
4×4; **1.000 exactos no pasa**. El resultado del chequeo estricto anterior
permanece visible aunque se acepte el presupuesto acordado.

Este presupuesto sólo corresponde a la migración. No cambia los goldens ni
las tolerancias de [V1](acceptance-v1.md) o de [Órdenes](orders-v1.md).

## Cambios

- Bevy queda fijado en `=0.20.0`, con su lockfile y backend wgpu 30.
- La UI usa picking y eventos de pulsación, liberación y cancelación. El
  adaptador conserva las acciones al presionar, las consultas `Changed`, el
  arrastre, los controles deshabilitados y la prioridad de ventanas sobre el
  mapa. Las etiquetas decorativas dejan pasar el puntero al control.
- El límite de `Time<Virtual>` se ajusta a la velocidad de simulación para
  conservar avance rápido, pausa y recuperación tras un frame lento.
- Los sprites de píxeles usan la extracción pública del pipeline de quads
  conservado para `Text2d`. Los modos sliced/tiled/masked conservan el backend
  de Bevy; los proxies del vidrio comparten el quad sin compresión.
- Las máscaras usan una cámara auxiliar ajustada al rango de profundidad de
  su geometría. Esto evita que las
  diferencias pequeñas entre padres e hijos se pierdan al proyectarlas con
  la cámara principal. La posición X/Y y el zoom se conservan.
- El shader de vidrio pasa a WESL. Los paquetes, sus smokes y la verificación
  de metadata incluyen el nuevo archivo.

## Evidencia y límites

La [evidencia de aceptación](evidence/bevy-020-migration-20261010.json) y su
[CSV por zoom](evidence/bevy-020-migration-20261010.csv) conservan métricas,
hashes de PNGs, binarios y fuentes. El ejecutable definitivo tiene SHA-256
`0d76b768d788d1c10a44f026a88198e2f05557e123b0def2dcdb602c8d7def3f`.

La pasada completa dio **0, 0, 0, 0, 512 y 447 píxeles que empeoran** en las
seis escalas, respectivamente. Mejoraron 2.800, 1.568, 0, 96, 818 y 237
píxeles. El chequeo estricto sigue fallando en `4` y `8`; el presupuesto
acordado pasa en todos los zooms. Normal es idéntico al cliente anterior y
conserva 45 píxeles distintos frente a OpenTTD, cinco con delta de canal >64,
media 0,0024390625 y cobertura ausente cero. Esta comprobación de una captura
no recertifica V1 para la nueva SHA.

Los ocho perfiles de Órdenes tienen delta cero en los ROIs originales. El
seguimiento runtime pasó en los seis zooms: **3.454 frames** con cero
desajustes entre cámara y ancla del tren. Normal avanzó 13.256 ticks, visitó
102 teselas y recorrió las ocho direcciones; las otras escalas muestrean el
movimiento durante 359 frames cada una.

Pasaron 1.718 tests del cliente, con dos ignorados, y Clippy estricto de todos
sus targets. Se conserva la validación de los 3.072 tests del core sin cambios,
con seis ignorados. También pasaron formato, los 15 tests Python afectados,
compilación del manifiesto Python, metadata de release, smoke de assets y
documentación de paridad. No se afirma CI remota verde.

La fixture es `save/Kale_TitleGame.sav`, SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`;
centro `132,2`, resolución 800×600, raster estático limpio, OpenGFX 8bpp y seis
escalas `0.25`, `0.5`, `1`, `2`, `4`, `8`. El oracle de exportación está fijado
en OpenTTD `c2661164bcb6cbf5ab97b56ccbee7506a3b26833`. Es el checkout
instrumentado del proyecto; no se presenta como un binario upstream limpio.

Las pérdidas que quedan se concentran en muestreo, superposiciones y vidrio.
Los zooms alejados ya tenían divergencias frente a OpenTTD antes de esta
migración. El presupuesto mide cuánto empeoran, no afirma que todo el frame
quede a menos de 1.000 píxeles de la imagen nativa.

El [recorrido controlado de Kale](kale-train-follow.md) conserva las señales y
comprueba carga, llegada, transferencia y regreso del tren 2902. Su prueba de
cámara compara el centro con el ancla renderizada del tren en cada frame y
conserva el zoom. No certifica la flota completa ni equivalencia de ticks con
OpenTTD.

## Reproducción

Con tres directorios que contienen `SCALE/world.png`:

```bash
python3 scripts/compare_renderer_upgrade.py \
  --baseline artifacts/bevy020/baseline-physical \
  --candidate artifacts/bevy020/fitted-final-hw \
  --oracle artifacts/bevy020/native-retry \
  --max-worsened-pixels-exclusive 1000 \
  --report artifacts/bevy020/fitted-final.json \
  --csv artifacts/bevy020/fitted-final.csv
```

Omitir el presupuesto ejecuta el chequeo estricto. Las capturas runtime de
Órdenes usan perfiles efímeros ES/EN × 1280×720/1920×1080 × escala 1/2 y los
ROIs fijos existentes; no reemplazan sus referencias versionadas.
