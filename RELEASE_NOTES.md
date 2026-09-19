# openttdrs 0.1.0-alpha.1

**Borrador de notas; no acredita una release publicada.** Al revisar GitHub el
2026-09-19 no hay publicaciones. El candidato ya cumplió el [corte Primera
ruta](docs/parity/continuous-work-plan.md), incluido su smoke gráfico y sesión
de aceptación; el [dry-run de release](https://github.com/cavazquez/openttdrs/actions/runs/35431897233)
también aprobó. Publicar el tag y la release sigue requiriendo una decisión
explícita.

Notas preparadas para la primera alpha pública. Incluye un cliente isométrico jugable,
servidor dedicado lockstep y herramientas headless de paridad con OpenTTD 15.3.

## Qué probar

- Crear una partida procedural y construir redes road/rail.
- Señales block/path y reservas PBS básicas.
- Economía, 11 cargas temperate, órdenes y CargoDist.
- Cargar saves JSON propios o importar parcialmente `.sav` / `.ottdmap`.
- Servidor dedicado `openttdrs-dedicated` y cliente `--server` / `--client`.

## Instalación

1. Descargá el archivo de tu plataforma y verificá el `.sha256` asociado.
2. Extraelo completo; `assets/` y `static/` deben quedar junto al ejecutable.
3. Ejecutá `openttdrs-client` (`openttdrs-client.exe` en Windows).

En Linux se requieren las bibliotecas de ventana/audio indicadas en el README.
Los binarios de esta alpha no están firmados ni notarizados.

## Estado y límites

Esta release es para pruebas, no una afirmación de paridad total. NewGRF,
multiplayer, barcos, aeronaves, UI y round-trip `.sav` continúan parciales. Ver
`docs/PARIDAD.md` en el código fuente etiquetado para el inventario exacto.

Los paquetes incluyen OpenGFX, OpenSFX y OpenMSX libres; atribuciones y licencias
están en `THIRD_PARTY_ASSETS.md` y `LICENSE`.
