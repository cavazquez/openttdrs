# PATS `vehicle.wagon_speed_limits` (#385)

Última actualización: 2026-09-06.

## Alcance cerrado

El booleano nativo `PATS.vehicle.wagon_speed_limits` se importa, hidrata en
`ConstructionSettings`, conserva el default `true` de OpenTTD y vuelve a
escribirse como `SLE_BOOL`. La reconstrucción de consistes usa el valor
persistido: con el setting activo cada unidad puede limitar la velocidad
máxima; con el setting desactivado los vagones no introducen ese mínimo. Los
wrappers legacy de `ConsistChanged` mantienen el comportamiento por defecto
activo para no romper callers que no poseen `GameState`.

Los call sites que sí tienen estado —carga de SAV, migración, compra/acople,
autoreemplazo, `SetFreightTrains`, transferencia de carga y movimiento— pasan
la preferencia hidratada antes de recalcular los cachés del consist.

## Oracle OpenTTD 15.3

`reference/openttd-15.3-oracle/src/table/settings/game_settings.ini:292-298`
declara el setting como `SDT_BOOL`, con default `true` y callback nativo
`UpdateConsists`. `reference/openttd-15.3-oracle/src/train_cmd.cpp:184-188`
aplica el mínimo de velocidad sólo cuando la unidad no es un vagón o el bool
está activo, y además excluye `UsesWagonOverride(u)`.

El core reproduce la condición vanilla de wagon frente a locomotora. El
override de vagón específico de NewGRF (`UsesWagonOverride`) todavía no se
integra en esta condición; por eso este issue cierra el setting SAV ejecutable,
no la paridad completa de callbacks/runtime NewGRF.

La candidata Rust del smoke tuvo SHA-256
`63b2bef3ac7e6251557e7a838439bcf83ceaaef7aa03060f5ff4657fde0938ee`.
OpenTTD 15.3 la re-guardó como un SAV de 8480 bytes con SHA-256
`fea5d36cf9d7932aedb02e49637a90b4021e623dc279791c19c58b4098c84695`; el
importador volvió a leer `false`.

## Regresiones

- default nativo y parser de PATS ausente/presente;
- wire PATS, round-trip `SavGame`/`GameState` y mutación posterior a importar;
- velocidad de consist limitada a 40 con el setting activo y a 160 cuando se
  desactiva en un wagon de 40 junto a una locomotora de 160;
- fixture rico y contrato `sav_openttd_roundtrip_subset` para el re-guardado de
  OpenTTD 15.3.

Gate publicado: `cargo fmt --all -- --check`, clippy core/client con
`-D warnings`, core 2083 tests, cliente 1105 pasados y 2 ignorados, matriz de
docs y `git diff --check`, todos verdes.

El issue padre [#328](https://github.com/cavazquez/openttdrs/issues/328)
permanece abierto por las demás tablas, settings y semánticas runtime.
