# Transferencia de carbón V1 contra OpenTTD 15.3

Actualizado: 2026-10-02.

El contrato V1-PKT de [#587](https://github.com/cavazquez/openttdrs/issues/587)
fija un recorrido Temperate de 64×64 con una mina, una parada de transferencia,
una central eléctrica y dos camiones. Antes de comenzar se construyen mediante
comandos públicos la carretera, el depósito, las tres paradas, los vehículos y
sus órdenes. No se añade carga, vehículos ni pagos después del arranque.

La inflación de pagos queda fijada a `65536` y no hay NewGRF dinámicos. El
primer camión carga en la mina y hace `Transfer`; el segundo sólo puede tomar
el packet que llegó desde otra estación y lo entrega a la central. Una carga
primaria creada directamente en la parada intermedia sigue rechazada: no abre
una segunda mina implícita.

## Ledger y conservación

`cargo_units_delivered` conserva su significado histórico de unidades
descargadas, incluidas las transferencias. El nuevo
`cargo_units_final_delivered` cuenta sólo la entrega final y permite comprobar,
sin mezclar ambos eventos, la igualdad exacta:

```text
stock inicial + producido = esperando + a bordo + entregado final
```

El test ejecuta el mismo log dos veces y exige el mismo hash canónico, la misma
traza y la misma igualdad. En la entrega no se vuelve a insertar el packet en
la cola de la estación: hacerlo acreditaría la central y duplicaría carga
física.

El primer tramo no aumenta `cargo_income_earned` ni `route_profit`. Con un
único trasbordo, su `feeder_share` es el 75 % entero del ingreso nativo. El
tramo final acredita una vez el bruto; el ledger verifica exactamente
`bruto = entregador + feeder`.

## Oráculo acotado

`scripts/temperate_payment_oracle.py` compila el cuerpo literal nativo de
`GetTransportedGoodsIncome` del pin OpenTTD 15.3. Además del corpus V1-PAY,
conserva dos vectores de pago: `transfer COAL 4 20 7 57` y
`final COAL 4 40 24 107`. Sus edades son entradas elegidas para la fórmula,
no una cronología medida ejecutando camiones en el simulador nativo.
No se usa el cálculo Rust para generar esos valores.

La traza está versionada en
`crates/openttdrs-core/tests/fixtures/parity/coal_transfer_15_3.tsv`; su
SHA-256 es
`495318d32dc2480d41c546e43b611ed0806f07c8582e03701c3fadb2591d9132`.
Su procedencia acompaña la fixture y fija el commit `15.3`
`14ec60f248547d4d062a1160f0fc26d742319888`, los hashes de las fuentes nativas
y el porcentaje de feeder usado por el contrato.

`coal_transfer_income_15_3.tsv` amplía el mismo cuerpo C++ a 512 entradas:
cuatro unidades de carbón, distancias 20/40 y edades 0–255. Su procedencia
acompaña el corpus; `--check` reproduce los tres corpus y sus hashes.
La prueba de recorrido consulta el pago nativo con la edad real del packet,
mantiene cantidades/distancias fijas y exige el ledger exacto y la
conservación física. Otra regresión compara los 512 pagos con Rust y
conserva ambos vectores originales. Tras corregir los giros de dársena,
la edad final pasó de 24 a 25: imponer 24 confundía una entrada económica
del harness con un resultado de una partida nativa que no se ejecutó.

```bash
python3 scripts/temperate_payment_oracle.py reference/openttd-upstream --check
cargo test -p openttdrs-core --test v1_coal_transfer
```

Quedan fuera la cronología nativa del recorrido completo, rutas CargoDist
complejas, varios hubs, reparto multicompañía, NewGRF y la equivalencia
económica global. Este contrato verifica los pagos para las entradas medidas
en el port, su ledger y su conservación física.
