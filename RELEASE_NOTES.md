# openttdrs 0.1.0-alpha.6

**Alpha del 2026-10-11.** Esta prerelease reúne los paquetes de escritorio
para Linux x86_64, Windows x86_64 y macOS arm64 en
[GitHub Releases](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.6)
y el Snap para Linux amd64 en el canal
[`latest/edge`](https://snapcraft.io/openttdrs). Incluye el cliente gráfico,
el servidor dedicado lockstep y los assets libres necesarios.

## Novedades

- Migración del cliente a Bevy 0.20.0, incluyendo controles de UI, tiempo
  virtual, extracción de sprites y el shader del vidrio.
- El recorrido controlado del tren de Kale corrige paradas, transferencia de
  carga, reservas pendientes y reintentos de señales PBS.
- Los recorridos de bus y camión de Kale verifican carga, descarga, servicio
  en depósito y continuidad al guardar y cargar durante el viaje. Las paradas
  físicas de estaciones compuestas se resuelven correctamente en esos casos.
- Las conexiones de los puentes corrigen una diferencia de altura de tres
  píxeles y el suelo que tapaba sus cabeceras. La comprobación contra OpenTTD
  cubre las alturas de dibujo de 428 cabeceras y 647 segmentos del mapa Kale.
- La migración y las escenas verificadas de vehículos y puentes pasan en los
  seis zooms con el criterio acordado de menos de 1.000 píxeles que empeoren
  por imagen respecto de la versión anterior. Las diferencias heredadas
  permanecen documentadas; esta tolerancia no certifica paridad visual total.

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
   [prerelease](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.6)
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

Las comprobaciones de esta entrega están en `docs/parity/bevy-020-migration.md`,
`docs/parity/kale-train-follow.md`, `docs/parity/kale-bus-follow.md`,
`docs/parity/kale-truck-follow.md` y `docs/parity/kale-bridge-connections.md`.

Los paquetes incluyen OpenGFX, OpenSFX y OpenMSX libres. Sus atribuciones y
licencias están en `THIRD_PARTY_ASSETS.md` y `LICENSE`.
