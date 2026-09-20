# Pagos Temperate contra OpenTTD 15.3

Actualizado: 2026-09-20.

El contrato V1-PAY verifica sólo `GetTransportedGoodsIncome` de las once
cargas Temperate vanilla, sin NewGRF y con `inflation_payment = 65536`. El
corpus fijo combina cantidades `0, 1, 100`, distancias `1, 32` y edades
`0, 30, 100`: **198 casos**.

## Oráculo reproducible

`scripts/temperate_payment_oracle.py` extrae el cuerpo literal de
`GetTransportedGoodsIncome` desde `src/economy.cpp` del checkout OpenTTD
fijado, lo compila en un binario temporal con stubs de `CargoSpec` y toma las
tasas/períodos de `src/table/cargo_const.h`. El harness no reimplementa la
fórmula ni invoca código Rust para producir el lado de referencia.

La fixture versionada es
`crates/openttdrs-core/tests/fixtures/parity/temperate_payment_15_3.tsv`; su
SHA-256 es
`097953112a928e2a471939dd865fa6b7ae04c05c5099824b3ef34ec26cad35a6`.
La procedencia junto a la fixture liga ese hash al commit
`14ec60f248547d4d062a1160f0fc26d742319888` (tag `15.3`) y a los hashes de
ambos archivos nativos. El generador rechaza un checkout con esos archivos
modificados.

```bash
./scripts/fetch-openttd-reference.sh
python3 scripts/temperate_payment_oracle.py reference/openttd-upstream --check
cargo test -p openttdrs-core transported_goods_income_matches_openttd_15_3_temperate_oracle
```

La regresión Rust lee sólo la tabla externa, ejecuta el cálculo productivo y
reporta el primer cargo, cantidad, distancia, edad, valor esperado y valor
actual que diverjan. El chequeo de tablas generadas de CI vuelve a ejecutar el
oráculo después de recuperar el pin OpenTTD.

## Corrección acotada

El primer caso divergente fue `PASS`, una unidad, distancia `1`, edad `0`:
OpenTTD devuelve `0` y el código Rust imponía un mínimo artificial de `1`.
Se eliminó únicamente ese piso después del corrimiento entero. No se modifican
climas alternativos, inflación variable, ratings, CargoDist ni callbacks
NewGRF.
