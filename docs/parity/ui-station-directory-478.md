# Paridad UI: directorio de estaciones (#478)

Fecha de implementación: 2026-09-07  
Issue: [#478](https://github.com/cavazquez/openttdrs/issues/478)  
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

`StationDirectory` tenía su título base catalogado, pero los controles de
compañía, tipo de instalación y carga permanecían en español. El estado vacío
y los fragmentos dinámicos de cada fila (`Camión`, `Muelle`, `Aeropuerto`,
`rating`, `espera`, además de los nombres de fallback por coordenadas) tampoco
seguían el locale activo. Eso hacía que cambiar a inglés dejara una ventana
mixta, aunque el orden, los filtros y la navegación funcionaran.

## Corrección

- `sync_station_directory` consulta `ClientPreferences::locale()` y traduce
  los labels de filtros y el estado vacío mediante el catálogo compartido.
- Los tipos de parada y las métricas de rating/espera se generan en el locale
  activo; el cambio de locale invalida la caché para redibujar las filas sin
  mutar su identidad.
- Nombres personalizados de estaciones y compañías, coordenadas, IDs, orden,
  filtros y navegación permanecen datos del juego y no se traducen.

## Regresión y alcance

La regresión cubre los labels de fallback en español e inglés, las métricas
dinámicas y la preservación de nombres personalizados. Los catálogos de
labels NewGRF o de cargos siguen siendo datos literales cuando no existe una
clave vanilla; la paridad completa de catálogos y textos generados continúa en
el parent #331.

Validación publicada: formatter, clippy estricto de core/cliente, tests de
core/cliente, gate de documentación de paridad y `git diff --check`.
