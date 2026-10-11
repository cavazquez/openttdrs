# Alpha.6: cimientos base fuera del checkout

Fecha: **2026-10-11**. Alcance: los cimientos base Action5 y la escena de
conexiones de puentes de Kale al ejecutar un paquete desde un perfil vacío.
La publicación de esta alpha se registra por separado de la corrección.

## Divergencia reproducida

El primer Snap alpha.6, construido desde `5b484690`, pasaba `--check-assets`
y el smoke del menú con assets de sólo lectura. Al cargar Kale fuera del
checkout, las cubiertas de vidrio se volvían negras: 4.818 píxeles cambiados
en `1×` y hasta 26.176 píxeles empeorados en `0.25×` respecto de la escena
aceptada. Esperar 240 frames producía la misma captura que esperar 40.

El mismo binario y los mismos assets dibujaban correctamente desde el
checkout. Añadir sólo el directorio de shaders al perfil no corregía la
imagen; hacer accesible `assets/opengfx/` sí. La traza de archivos identificó
la lectura relativa de `opengfx-8.0/ogfxe_extra.grf`. La traza ECS registró
16.068 sprites con ese recurso y 15.729 sin él. El atlas y la cobertura del
vidrio eran idénticos; faltaban entidades de cimientos y cambiaba el orden
geométrico de padres e hijos.

Los checks previos de raster ejecutaban el cliente con `cwd` y assets del
repositorio. Por eso conservaban un GRF descargado que no formaba parte del
paquete. El menú tampoco cubre esta geometría de estaciones y puentes.

## Corrección y regresión

Los 90 sprites de cimientos base se leen ahora de los bytes de OpenGFX 8.0
incluidos en el core. Se elimina la búsqueda implícita en el directorio de
trabajo y en la ruta del checkout usado para compilar. Los reemplazos del
stack NewGRF explícito se aplican después, como antes.

El GRF, licencia y readme coinciden byte por byte con el archivo oficial
`opengfx-8.0-all.zip`. Sus hashes y créditos están en
`crates/openttdrs-core/assets/opengfx-8.0/PROVENANCE.md`. La regresión comprueba
los 90 slots decodificados en los cuatro climas; conserva los slots nativos
intencionalmente transparentes.

La captura controlada del cliente corregido, con los assets de sólo lectura
del paquete y perfiles vacíos fuera del checkout, coincide exactamente con
la escena aceptada en `0.25`, `0.5`, `1`, `2`, `4` y `8`: cero píxeles cambiados.
Esto conserva las diferencias heredadas respecto de OpenTTD documentadas en
[las conexiones de puentes](kale-bridge-connections.md).

## Reproducir sobre el Snap construido

Extraer el `.snap` en un directorio nuevo, quitar sus permisos de escritura y
usar el launcher incluido; cada zoom recibe su propio perfil:

```bash
unsquashfs -q -d artifacts/alpha6-snap-check/root \
  artifacts/alpha6-snap/openttdrs_0.1.0-alpha.6_amd64.snap
chmod -R a-w artifacts/alpha6-snap-check/root
python3 scripts/capture_kale_bridge_heads.py \
  --binary artifacts/alpha6-snap-check/root/bin/openttdrs-client \
  --asset-root artifacts/alpha6-snap-check/root \
  --launcher artifacts/alpha6-snap-check/root/bin/openttdrs-launch \
  --isolated-cwd --out artifacts/alpha6-snap-check/captures
python3 scripts/compare_renderer_upgrade.py \
  --baseline artifacts/transport-follow/bridges/masked-final \
  --candidate artifacts/alpha6-snap-check/captures \
  --oracle artifacts/transport-follow/bridges/native-flat \
  --report artifacts/alpha6-snap-check/comparison.json \
  --csv artifacts/alpha6-snap-check/comparison.csv \
  --max-worsened-pixels-exclusive 1000
```

La escena usa `Kale_TitleGame.sav`, cámara `(204,117)`, resolución `800×600`,
simulación pausada, zoom nativo y 40 frames de estabilización. La ejecución
gráfica requiere un display y Vulkan. La extracción read-only comprueba
launcher y assets; no equivale a instalar el Snap bajo AppArmor.

Este cierre cubre los recursos base y esa escena. No certifica todos los
cimientos, la paridad global del renderer, confinamiento instalado ni 30 FPS.
