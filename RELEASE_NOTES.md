# openttdrs 0.1.0-alpha.1

**Publicada el 2026-09-19.** La alpha está disponible como
[prerelease de GitHub](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
y como [Snap Store `latest/edge`](https://snapcraft.io/openttdrs) para Linux
amd64. El [corte Primera ruta](docs/parity/continuous-work-plan.md), su smoke
gráfico y la sesión de aceptación ya están completados; el
[workflow de release](https://github.com/cavazquez/openttdrs/actions/runs/35453383079)
publicó los paquetes de escritorio.

Incluye un cliente isométrico jugable, servidor dedicado lockstep y herramientas
headless de paridad con OpenTTD 15.3.

## Qué probar

- Crear una partida procedural y construir redes road/rail.
- Señales block/path y reservas PBS básicas.
- Economía, 11 cargas temperate, órdenes y CargoDist.
- Cargar saves JSON propios o importar parcialmente `.sav` / `.ottdmap`.
- Servidor dedicado `openttdrs-dedicated` y cliente `--server` / `--client`.

## Instalación

Para Linux amd64, el canal alpha del Snap Store es:

~~~bash
sudo snap install openttdrs --channel=latest/edge
openttdrs
~~~

El Snap incluye assets y conserva las partidas JSON en
`~/snap/openttdrs/common/save/`. El servidor dedicado se inicia con
`openttdrs.dedicated`.

Para Linux x86_64, Windows x86_64 o macOS arm64:

1. Descargá el archivo de tu plataforma desde la
   [prerelease](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
   y verificá el `.sha256` asociado.
2. Extraelo completo; `assets/` y `static/` deben quedar junto al ejecutable.
3. Ejecutá `openttdrs-client` (`openttdrs-client.exe` en Windows).

Los archivos de GitHub para Linux requieren las bibliotecas de ventana/audio
indicadas en el README. El Snap usa confinamiento estricto; `latest/edge` es
un canal de pruebas, no estable. Los binarios de GitHub de esta alpha no están
notarizados.

## Estado y límites

Esta release es para pruebas, no una afirmación de paridad total. NewGRF,
multiplayer, barcos, aeronaves, UI y round-trip `.sav` continúan parciales. Ver
`docs/PARIDAD.md` en el código fuente etiquetado para el inventario exacto.

Los paquetes incluyen OpenGFX, OpenSFX y OpenMSX libres; atribuciones y licencias
están en `THIRD_PARTY_ASSETS.md` y `LICENSE`.
