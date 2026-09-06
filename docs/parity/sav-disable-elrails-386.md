# PATS `vehicle.disable_elrails` (#386)

Última actualización: 2026-09-06.

## Alcance cerrado

`PATS.vehicle.disable_elrails` se importa y exporta como `SLE_BOOL`, con el
default nativo `false`, dentro de `ConstructionSettings`. Al activarlo, el
comando de compra trata las locomotoras eléctricas como aptas para rail normal
y una conversión Electric → Rail existente es un no-op, igual que la
normalización de `OpenTTD` cuando la red eléctrica está desactivada. Los call
sites de simulación ya conservan la compatibilidad geométrica Rail/Electric;
este issue no presenta esa compatibilidad general como una nueva regla de
pathfinding.

La función pública `engine_compatible_with_rail_setting` mantiene la API
legacy (default activo) y expone la variante que recibe la preferencia
persistida. Las regresiones cubren parser/default, round-trip/mutación PATS,
compra de una AsiaStar sobre rail normal y conversión sin cobro.

## Oracle OpenTTD 15.3

`reference/openttd-15.3-oracle/src/table/settings/game_settings.ini:300-307`
declara el bool con default `false` y callback `SettingsDisableElrail`.
`src/elrail.cpp:581-605` mueve motores destinados a electrificación a rail
normal y actualiza trenes existentes; `src/rail_cmd.cpp:1577-1580` trata
Electric → Rail como no-op cuando está desactivado.

La candidata Rust del smoke tuvo SHA-256
`d2af8e4ccc5e016adc210cbdbcb2bcde167642ca9730d1922477c5112b0c3b9f`.
OpenTTD 15.3 la re-guardó como un SAV de 8480 bytes con SHA-256
`28da314ff3b70e7f04b329b80304cfce5cfd069bdc863bc28d692f9c2d73e8c6`; el
importador volvió a leer `true`.

El renderizador propio todavía decide la catenaria desde el tipo de tesela y
sus sprites; este issue no afirma que el overlay visual global desaparezca en
cada nivel de zoom. Esa parte queda como residual de raster/NewGRF (#326/#329).

Gate publicado: `cargo fmt --all -- --check`, clippy core/client con
`-D warnings`, core 2089 tests, cliente 1105 pasados y 2 ignorados, matriz de
docs y `git diff --check`, todos verdes.

El issue padre [#328](https://github.com/cavazquez/openttdrs/issues/328)
permanece abierto por el resto de settings, pools y runtime SAV.
