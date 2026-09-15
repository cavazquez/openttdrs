# Contrato de captura raster focalizada

`world-draw` prueba decisiones de dibujo. Este contrato complementa esa
evidencia con los píxeles compuestos por OpenTTD y por `openttdrs`, en una
misma tesela, resolución y zoom. La escala se fija en ambos motores: no se
compara una candidata alejada con una referencia normal.

## Ejecución

```bash
SAV=save/Kale_TitleGame.sav
./scripts/compare_focused_world_screenshot.sh "$SAV" /tmp/kale-tunnel 189,126 1280x720 1
```

El directorio de salida contiene cuatro artefactos:

- `reference.png`: viewport de OpenTTD 15.3 parcheado a la escala solicitada.
- `candidate.png`: viewport de `openttdrs`, sin UI, HUD, rótulos, vehículos
  ni audio.
- `diff.png`: negro para píxeles iguales, color amplificado para diferencias y
  magenta cuando falta cobertura de la candidata.
- `report.json`: hashes, save, centro, resolución, perfil gráfico, métricas y
  traducción de cámara encontrada. Su campo `capture.openttd_zoom` declara el
  `ZoomLevel` nativo efectivo (`In4x`…`Out8x`), no una etiqueta fija.

El argumento `escala` (o `OPENTTDRS_WORLD_SCREENSHOT_SCALE`) se propaga a ambos
capturadores con esta convención compartida:

| Escala ortográfica openttdrs | Zoom nativo OpenTTD |
|---:|---|
| `0.25` | `In4x` |
| `0.5` | `In2x` |
| `1` | `Normal` |
| `2` | `Out2x` |
| `4` | `Out4x` |
| `8` | `Out8x` |

El exportador nativo acepta sólo esos seis valores; una escala inválida aborta
en vez de degradar silenciosamente a zoom normal. Por defecto activa el perfil
`clean-static`: pausa ambas partidas,
desactiva animaciones y oculta rótulos y vehículos. Eso hace que el diff mida
geografía, terreno, infraestructura y edificios, no el instante en que cada
motor actualizó una unidad. Para investigar sprites de vehículos o animaciones
en particular se puede usar `OPENTTDRS_WORLD_SCREENSHOT_CLEAN=0`; el valor
`0` (también `false`, `no` u `off`) desactiva la limpieza en ambos lados. El
`report.json` lo registra como perfil `dynamic` y no debe mezclarse con un
resultado estático. La referencia actual usa OpenGFX 8bpp y el orquestador rechaza una
candidata 32bpp por defecto: comparar perfiles distintos no demuestra un fallo
de render. `OPENTTDRS_WORLD_SCREENSHOT_ALLOW_GFX_MISMATCH=1` sólo habilita una
exploración explícitamente no comparable.

En `clean-static`, el candidato también suprime HUD, diagnóstico, gizmos de
industria/estación y Link Graph aunque estén activados en preferencias locales
o mediante variables de entorno. Esas capas sirven para depurar, no son parte
del raster comparable.

### Traza diagnóstica del orden de composición

Cuando una diferencia depende de la superposición, se pueden activar trazas
efímeras junto con una captura normal:

```bash
OPENTTDRS_WORLD_SCREENSHOT_SORT_OUT=/tmp/reference-sort.jsonl \
  ./scripts/export_openttd_world_screenshot.sh "$SAV" /tmp/reference.png \
    reference/openttd-upstream/build/openttd 189,126 1280x720
OPENTTDRS_VIEWPORT_SORT_TRACE_OUT=/tmp/candidate-sort.json \
  ./scripts/export_openttdrs_world_screenshot.sh "$SAV" /tmp/candidate.png 189,126 1280x720 1
```

La traza nativa JSONL registra el vector de parents después de
`_vp_sprite_sorter` para los segmentos de la captura. La candidata registra el
scope seleccionado, los parents de entrada, su orden resultante y profundidad.
Cuando un `SpriteCombine` atraviesa más de una banda, `local_proxies` enumera
las copias recortadas que se ordenan dentro de cada banda sin entrar al sorter
global. Cada proxy conserva `band`, `source_child`, `original_parent`,
`sprite_id`, `world_bounds` y las profundidades de origen/final; por eso el
conteo efectivo para investigar una promoción es `parents + local_proxies`.
El campo es opcional para mantener compatibilidad con trazas anteriores.
El escritor deduplica snapshots consecutivos por una firma que incluye scope,
orden, profundidades y proxies; una captura que sólo espera frames estables no
debe reserializar el mismo documento una y otra vez.
Son instrumentos de diagnóstico: no sustituyen `report.json`, no se publican
como baseline y no permiten declarar paridad por una sola región.

Para separar una promoción por clipping de una caja realmente ausente se puede
resumir la traza nativa junto con la candidata:

```bash
python3 scripts/analyze_viewport_sort_bands.py \
  /tmp/reference-sort.jsonl \
  --candidate-sort /tmp/candidate-sort.json \
  --json-report /tmp/viewport-sort-bands.json
```

El informe cuenta segmentos, parents acumulados, cajas de mundo repetidas con
distintos sprites y cajas que no aparecen en la pasada global candidata. Una
misma caja con varios sprites no implica un asset divergente: puede ser el
primer child visible de una secuencia `StartSpriteCombine` en otra banda.

## Métricas y registro

El comparador calcula primero el diff sin corrección y luego busca una
traslación entera de hasta ocho píxeles de la candidata. El reporte conserva
ambos resultados:

- `metrics.raw`: diferencia con las cámaras tal como se capturaron.
- `metrics.aligned`: diferencia tras la traslación elegida.
- `alignment.candidate_translation`: desplazamiento aplicado a la candidata
  (`+x` derecha, `+y` abajo).

La traslación no se usa para declarar paridad. Si no es `[0, 0]`, hay que
investigar el mapeo cámara/viewport antes de atribuir todas las diferencias a
sprites. Las zonas desplazadas fuera de la candidata cuentan como diferencia
máxima; el registro nunca descarta bordes para mejorar artificialmente el
resultado.

## Uso correcto

El diff raster es el control de composición, no una autorización para inferir
la causa sin las capas anteriores. Antes de tocar sprites hay que conservar el
orden de evidencia:

1. `world-raw` para bytes del `.sav`.
2. `world-semantic` para tipo, orientación, railtype y vecinos.
3. `world-draw` para sprite, paleta, geometría y orden.
4. Esta captura para composición, clipping, cámara, atlas y capas no trazadas.

Conservar `report.json` junto con un issue o una regresión: el SHA-256 de la
partida y de ambas imágenes permite repetir el mismo diagnóstico aunque el
save de trabajo cambie después.

El estado cuantitativo de la última corrida comparable se publica sólo en
[PARIDAD.md](../PARIDAD.md#evidencia-visual-raster-vigente). Un `world-draw`
contenido no puede sustituir esta captura: ambos contratos miden etapas
distintas.

Una entrada cuantitativa vigente debe enlazar un reporte versionado y declarar
como mínimo: fecha, SHA-256 del `.sav`, versión y pin oficial de OpenTTD, commit
del oracle instrumentado, commit candidato, perfil gráfico, centro, resolución,
escala/`ZoomLevel`, alineación y píxeles distintos/total. La fila normal puede
ser el baseline global de una fixture; las demás escalas son diagnósticos y no
se comparan entre sí. Un número focal de un commit anterior debe quedar marcado
como histórico o eliminarse de la narrativa canónica: nunca puede presentarse
como una segunda métrica vigente.
