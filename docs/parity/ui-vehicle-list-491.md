# Evidencia de paridad: lista global de vehículos (#491)

Fecha: 2026-09-07
Issue: [#491](https://github.com/cavazquez/openttdrs/issues/491)
Parent: [#331](https://github.com/cavazquez/openttdrs/issues/331)

## Divergencia observada

`VehicleList` construía en español sus textos dinámicos y sus controles aun
cuando la preferencia de idioma estaba en inglés. El título por tipo, filtros,
orden, acciones, filtro por estación, estados `En marcha`/`Detenido`/`Sin
ruta`, la edad, el grupo, el botón de inicio/parada y los estados vacíos no
seguían el locale activo. Eso dejaba la ventana inconsistente con las fichas
de vehículo y con el resto de la UI localizada.

## Corrección

- `VehicleListState` mantiene el locale sincronizado con
  `ClientPreferences`; el sistema dedicado evita exceder los parámetros de
  la consulta ECS de la lista.
- Título, estación, filtros, orden, acciones, estados, grupo, botón y mensajes
  vacíos pasan por el catálogo `i18n`.
- La fila conserva nombre, ID implícito, edad, velocidad, coordenadas,
  selección y status, cambiando sólo la unidad textual de edad (`a`/`y`) y el
  status localizado.
- La caché incluye el locale para reconstruir las filas inmediatamente al
  cambiar de idioma.
- Las cadenas sin entrada (nombres de compañías, vehículos y grupos
  NewGRF/custom) permanecen literales.

## Regresión

Se añadieron pruebas unitarias para verificar la localización de estado/título/
fila sin mutar identidad ni coordenadas, para comprobar que el recurso de
estado sigue `ClientPreferences` y para que los controles estáticos se
actualicen al cambiar de locale.

## Validación

- `cargo fmt --all -- --check`
- `cargo check -p openttdrs-client --bin openttdrs-client`
- `cargo test -p openttdrs-client --bin openttdrs-client vehicle_list`
- `cargo test -p openttdrs-client --bin openttdrs-client i18n::tests`
- `./scripts/check_parity_docs_fresh.sh`

## Residual

La ventana de grupos dedicada, acciones masivas de flota, criterios avanzados
de orden y los catálogos upstream restantes continúan en #331. Este issue no
pretende traducir nombres procedentes de NewGRF ni alterar datos del juego.
