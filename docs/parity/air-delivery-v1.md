# V1-AIR — primer servicio Country pagado

El contrato [#594](https://github.com/cavazquez/openttdrs/issues/594) fija un
solo servicio de pasajeros de ala fija. No certifica todos los aeropuertos ni
el tráfico aéreo general.

## Fixture y log público

`crates/openttdrs-core/tests/v1_air_delivery.rs` usa un mapa Temperate plano
de 64×64, semilla `0x5940_0064`, año 1950, desastres y averías deshabilitados.
Coloca por comandos dos aeropuertos `AirportSpecId::Small` (el aeropuerto
Country nativo) con origen en `(4,4)` y `(42,42)`. Sus anclas de hangar son
`(7,4)` y `(45,42)`.

Antes de iniciar la operación, la fixture fija 50 pasajeros en el aeropuerto
de origen. No añade carga, vehículos, capacidad, posición ni pagos durante el
viaje. El log público compra un Dakota vanilla en el hangar, crea las órdenes
`carga completa → descarga` y lo inicia.

## Aceptación reproducible

```bash
cargo test --locked --offline -p openttdrs-core --test v1_air_delivery -- --nocapture
```

La corrida fija registra despegue en tick 57, salida del FTA de origen en 75,
entrada y aterrizaje FTA Country de destino en 1.374 y la primera entrega en
1.391. Entrega 8 pasajeros, acredita 303 de ingreso y termina con hash canónico
`14221659823881627150`.

En cada tick, el test exige que `Station::airport_blocks` sea exactamente la
unión de `airport_blocks_held` de los aviones cuyo
`airport_fta_station` apunta a esa estación: no hay bloques FTA huérfanos. La
prueba además verifica reserva y liberación en origen, entrada FTA y aterrizaje
en destino, conservación `demanda inicial = espera + a bordo + entrega`, crédito
por tipo Passenger y ausencia de crashes.

El mismo log se ejecuta dos veces; la secuencia discreta FTA/despegue/aterrizaje,
el tick y monto de la entrega, y el hash final deben coincidir exactamente. El
oráculo Helidepot preexistente sigue dentro de la suite normal.

## Límite explícito

Quedan fuera otros layouts FTA, múltiples aviones y colisiones, ruido/crashes,
helicópteros adicionales, NewGRF y comparación física externa con OpenTTD.
