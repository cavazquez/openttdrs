# openttdrs 0.1.0-alpha.5

**Alpha del 2026-10-02.** Esta prerelease reúne los paquetes de escritorio
para Linux x86_64, Windows x86_64 y macOS arm64 en
[GitHub Releases](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.5)
y el Snap para Linux amd64 en el canal
[`latest/edge`](https://snapcraft.io/openttdrs). Incluye el cliente gráfico,
el servidor dedicado lockstep y los assets libres necesarios.

## Novedades

- Mejor rendimiento con muchos vehículos: se reutilizan índices de flota,
  ocupación, depósitos y estaciones, además de sprites y proxies sin cambios.
- Las rutas de carretera y agua se conservan mientras la topología y el destino
  sigan vigentes. CargoDist calcula sus trabajos en workers y los publica en
  fechas previstas; sus flows y trabajos pendientes sobreviven a saves JSON.
- Los casos verificados de trenes, buses y camiones conservan movimiento por
  píxel, fracciones, separación de vagones y giros estacionarios. Carga,
  descarga y esperas mantienen los vehículos quietos antes de salir.
- Se corrigen detalles gráficos al cambiar zoom, orden de sprites, animaciones
  vanilla, humo y chispas eléctricas, conservando las decisiones RNG cuando el
  pool de efectos está lleno.
- Los builds de desarrollo generan menos símbolos y conservan archivo/línea
  en los backtraces. Las mediciones y sus límites están documentados en
  `docs/parity/performance-implementation.md`.

El protocolo propio de red es **v6**: cliente y servidor deben usar esta alpha.
Las mejoras de rendimiento no garantizan 30 FPS en todas las partidas.

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
   [prerelease](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.5)
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
