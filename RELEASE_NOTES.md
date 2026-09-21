# openttdrs 0.1.0-alpha.3

**Publicada el 2026-09-20.** Esta alpha reúne la distribución de escritorio
como [prerelease de GitHub](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.3)
y el [Snap Store `latest/edge`](https://snapcraft.io/openttdrs) para Linux
amd64. Incluye un cliente isométrico jugable, servidor dedicado lockstep y
herramientas headless de paridad con OpenTTD 15.3.

## Novedades destacadas

- Nueva partida ahora genera una semilla automática concreta y distinta en
  cada intento; la semilla se ve, se puede editar y se puede regenerar desde
  el menú.
- Empezar otra partida no hace volver el reloj de simulación a cero: conserva
  el tick ya alcanzado cuando éste es posterior al año de inicio elegido.
- El formulario de Nueva partida usa dos columnas en pantallas amplias, con
  mapa/inicio/dinero a la izquierda y semilla/mundo a la derecha. En pantallas
  angostas se pliega a una columna con scroll.
- El Snap `core24` incluye los PNG OpenGFX derivados antes del empaquetado y
  pasa el smoke sobre un mount de assets de sólo lectura.

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
   [prerelease](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.3)
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
