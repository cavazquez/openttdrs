# Contrato de captura raster focalizada

`world-draw` prueba decisiones de dibujo. Este contrato complementa esa
evidencia con los píxeles compuestos por OpenTTD y por `openttdrs`, en una
misma tesela, resolución y zoom normal.

## Ejecución

```bash
SAV=save/Kale_TitleGame.sav
./scripts/compare_focused_world_screenshot.sh "$SAV" /tmp/kale-tunnel 189,126 1280x720 1
```

El directorio de salida contiene cuatro artefactos:

- `reference.png`: viewport normal de OpenTTD 15.3 parcheado.
- `candidate.png`: viewport de `openttdrs`, sin UI, HUD, rótulos, vehículos
  ni audio.
- `diff.png`: negro para píxeles iguales, color amplificado para diferencias y
  magenta cuando falta cobertura de la candidata.
- `report.json`: hashes, save, centro, resolución, perfil gráfico, métricas y
  traducción de cámara encontrada.

El script fija `OPENTTDRS_MAP_SHOT_SCALE=1`, que equivale a `ZoomLevel::Normal`
de OpenTTD. Por defecto activa el perfil `clean-static`: pausa ambas partidas,
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
