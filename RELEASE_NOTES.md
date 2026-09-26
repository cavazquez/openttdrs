# openttdrs 0.1.0-alpha.4

**Publicada el 2026-09-26.** Esta prerelease reúne los paquetes de escritorio
para Linux x86_64, Windows x86_64 y macOS arm64 en
[GitHub Releases](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.4)
y el Snap para Linux amd64 en el canal
[`latest/edge`](https://snapcraft.io/openttdrs). Incluye el cliente gráfico,
el servidor dedicado lockstep y los assets libres necesarios.

## Novedades

- El mouse se comporta de forma más predecible: ventanas y controles conservan
  sus gestos, clic derecho y paneo no se confunden, y los arrastres de obra
  usan el punto real de liberación o se cancelan si la interacción se
  interrumpe.
- Construir sin fondos suficientes no altera el mapa. Las obras parciales
  informan qué teselas se rechazaron, por qué y cuánto se gastó.
- La escena isométrica animada del menú representa una partida. Aeronaves y
  barcos mantienen posiciones y orientaciones coherentes durante el trayecto.
- Se corrige la pérdida del primer clic sobre el mapa después de usar la barra
  de herramientas. Las capturas de partidas JSON también evitan una doble
  transición de menú.
- GitHub Actions reutiliza la caché del job Python para acelerar las
  comprobaciones repetidas.

## Instalación

Para Linux amd64, instalar o actualizar el Snap alpha:

~~~bash
sudo snap install openttdrs --channel=latest/edge
# Si ya estaba instalado:
sudo snap refresh openttdrs --channel=latest/edge
openttdrs
~~~

El Snap incluye sus assets, usa confinamiento estricto y guarda las partidas
JSON en `~/snap/openttdrs/common/save/`. El servidor dedicado se inicia con
`openttdrs.dedicated`.

Para Linux x86_64, Windows x86_64 o macOS arm64:

1. Descargá el archivo de tu plataforma desde la
   [prerelease](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.4)
   y verificá el `.sha256` asociado.
2. Extraelo completo; `assets/` y `static/` deben quedar junto al ejecutable.
3. Ejecutá `openttdrs-client` (`openttdrs-client.exe` en Windows).

Los archivos de GitHub no están firmados ni notarizados. `latest/edge` es un
canal de pruebas, no estable.

## Alcance

Esta alpha no afirma paridad total con OpenTTD. NewGRF, multiplayer, barcos,
aeronaves, UI y compatibilidad de ida y vuelta con `.sav` continúan parciales;
los contratos y límites verificados están en `docs/PARIDAD.md` y
`docs/parity/continuous-work-plan.md`.

Los paquetes incluyen OpenGFX, OpenSFX y OpenMSX libres. Sus atribuciones y
licencias están en `THIRD_PARTY_ASSETS.md` y `LICENSE`.
