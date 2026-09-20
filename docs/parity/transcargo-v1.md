# V1-AIT: primera entrega ferroviaria de TransCargo

Contrato acotado de [#596](https://github.com/cavazquez/openttdrs/issues/596).
Certifica una sola ruta de carbón de la IA propia; no intenta certificar NoAI,
Squirrel, estrategia general ni una red ferroviaria arbitraria.

## Fixture y límites

`v1_transcargo_delivery.rs` crea un mapa Temperate plano de 64×64 con seed
`0x5960_0064`, una mina de carbón en `(8,32)` y una central eléctrica en
`(48,32)`. La compañía humana queda inactiva y TransCargo recibe 2.000.000;
no hay vehículos, estaciones, infraestructura ni colas de construcción de IA
prefabricados. Desastres están desactivados y `max_routes=1` limita el
escenario a una ruta.

La prueba sólo llama `GameState::step()`: el scheduler planifica, construye,
compra el tren y crea sus órdenes mediante los comandos productivos. No
inyecta comandos, pagos ni carga para sustituir una decisión de la IA.

## Resultado reproducible

La primera entrega física de carbón a la central ocurre antes del límite de
40.000 ticks. Dos ejecuciones con el mismo seed producen exactamente el mismo
reporte:

```text
tick de primera entrega: 5688
costo neto de TransCargo: 51122
ingreso de la entrega: 278
órdenes: carga completa (10,32) -> descarga (46,32)
canonical_hash: 10541878588611481441
```

Al momento de esa entrega, la central registra la aceptación de carbón y el
tren de TransCargo conserva ambas órdenes. En cada tick, dinero, vehículos,
estaciones e infraestructura humanos se comparan exactamente contra una
ejecución gemela con la IA desactivada. De ese modo el contrato detecta una
mutación causada por TransCargo sin suprimir el cargo mensual fijo que OpenTTD
15.3 aplica normalmente a todas las compañías.

Los dos caminos de refit automático (en estación y en depósito) debitan la
compañía propietaria del vehículo, nunca el espejo de la compañía activa.

Desde la raíz del repositorio:

```sh
cargo test --locked --offline -p openttdrs-core --test v1_transcargo_delivery -- --nocapture
cargo test --locked --offline -p openttdrs-core --lib sim_step::cargo_transfer::tests::station_refit_charges_the_vehicle_owner_not_the_active_company -- --exact --nocapture
cargo test --locked --offline -p openttdrs-core --lib sim_step::vehicle_ops::tests::depot_order_refit_charges_vehicle_owner_not_active_company -- --exact --nocapture
```

El cierre de #596 exige CI remota verde para el SHA publicado. Quedan fuera
más rutas, estrategias, clima/seed alternativos, competencia humana activa y
todo runtime NoAI/Squirrel.
