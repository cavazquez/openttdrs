# Smoke gráfico nativo de paquetes V1

Los contratos [#601](https://github.com/cavazquez/openttdrs/issues/601) y
[#602](https://github.com/cavazquez/openttdrs/issues/602) validan el menú del
archivo distribuible real, no el checkout. `smoke_release_package.sh` extrae el
ZIP de Windows o el `tar.gz` macOS, arranca el cliente desde un cwd temporal y
fija `OPENTTDRS_ASSET_ROOT` a la raíz extraída. El perfil de preferencias y
saves también es temporal.

En Windows y macOS delega en
[`smoke_release_graphical.py`](../../scripts/smoke_release_graphical.py), que
no crea Xvfb, un backend offscreen ni un fallback headless. Ejecuta el binario
en la sesión gráfica nativa del runner para `es` y `en`, exige el marcador del
menú localizado, un PNG no vacío de **1280×720** y salida dentro de **60 s por
idioma**. Una sesión gráfica ausente, un panic, timeout, asset faltante o
captura plana produce `failed`, nunca `skip`.

En macOS el driver solicita sólo para esa captura una ventana nativa sin
bordes a pantalla completa. Así el framebuffer conserva 1280×720 incluso si
el menú y el dock reducen el área útil de una ventana decorada a 1280×653; no
recorta la imagen, no usa un display virtual y no altera el límite de 60 s.

## Evidencia

El directorio de artefactos contiene:

- `graphical/graphical-smoke.json`: SHA fuente, SHA-256 del paquete, sistema,
  driver/sesión declarados y resultado por idioma;
- `graphical/menu-es.png`, `graphical/menu-en.png` y sus logs;
- `check-assets.log`, `network.log`, `dedicated.log` y `package.sha256` del
  smoke común.

El workflow de release ejecuta este gate para Linux, Windows y macOS también
en `workflow_dispatch`; el job `publish` conserva su condición exclusiva para
tags. Así un dry-run puede generar evidencia sin crear tags ni publicar una
release.

## Reproducción

En un runner Windows o macOS con sesión gráfica real, después de construir el
paquete para esa plataforma:

```sh
OPENTTDRS_RELEASE_GRAPHICAL_SMOKE=1 \
OPENTTDRS_RELEASE_GRAPHICAL_TIMEOUT_SECONDS=60 \
OPENTTDRS_RELEASE_CANDIDATE_SHA=<sha-candidata> \
OPENTTDRS_RELEASE_SMOKE_ARTIFACT_DIR=dist/release-smoke-<plataforma> \
  ./scripts/smoke_release_package.sh <paquete.zip-o-tar.gz>
```

El contrato permanece pendiente hasta que el dry-run de la misma SHA publique
los artefactos aprobados para ambas plataformas. Los runners hosted pueden no
ofrecer una sesión apta para automatización gráfica; en ese caso el fallo y sus
logs son el bloqueo visible, y debe usarse un runner con sesión gráfica antes
de declarar verde el contrato.
