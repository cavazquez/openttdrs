# V1-GSCOUNT: `CargoDelivered` por tipo de carga

Contrato acotado de [#598](https://github.com/cavazquez/openttdrs/issues/598).
El objetivo propio GS-lite `CargoDelivered { cargo, min }` cuenta solamente
unidades de ese `cargo` entregadas físicamente por la compañía evaluada.

## Estado persistente y migración

`Company::cargo_units_delivered_by_type` es el ledger mínimo persistente:
un `CargoDeliveryLedger` indexado por ID de carga. Las rutas de descarga
acreditan el paquete sólo cuando existe entrega final física; una transferencia
o una parada de freight sin receptor no lo incrementa. La ruta de correo de
avión acredita asimismo sus unidades finales.

El campo nuevo lleva `#[serde(default)]`. Al leer JSON anterior queda vacío:
el agregado histórico `cargo_deliveries` cuenta eventos, no conserva tipo ni
unidades, y por tanto no se atribuye a carbón ni a ningún otro cargo.

## Evidencia reproducible

La regresión `cargo_goal_counts_only_the_requested_cargo_of_the_active_company`
verifica el contrato completo para una meta `Coal=10`:

- diez `Passengers` de la compañía activa dejan el progreso de carbón en 0;
- diez `Coal` de la rival tampoco lo avanzan;
- cinco `Coal` de la activa dan 5, otras cinco completan, y la rama separada
  fija el límite exacto 9/10 antes de la unidad final;
- un total legado de diez eventos sigue sin influir en el resultado.

La prueba de migración `legacy_delivery_total_does_not_seed_the_cargo_ledger`
elimina el campo nuevo de un JSON de compañía y confirma que ambos tipos siguen
en cero. La ruta V1 de dos camiones reutiliza su oráculo de carbón y comprueba
que su entrega física final queda en el ledger `Coal` de la compañía activa.

Desde la raíz del repositorio:

```sh
cargo test --locked --offline -p openttdrs-core gs::tests::cargo_goal_counts_only_the_requested_cargo_of_the_active_company -- --exact --nocapture
cargo test --locked --offline -p openttdrs-core --lib company::tests::legacy_delivery_total_does_not_seed_the_cargo_ledger -- --exact --nocapture
cargo test --locked --offline -p openttdrs-core --test v1_coal_transfer v1_two_truck_coal_transfer_conserves_cargo_and_matches_ledger_twice -- --exact --nocapture
cargo clippy --locked --offline -p openttdrs-core --lib --tests -- -D warnings
```

El cierre de #598 requiere que el SHA publicado que contiene estas pruebas
termine sus checks remotos vigentes en verde. Persistir progreso/noticias de
GS-lite a través de JSON sigue siendo el contrato separado
[#599](https://github.com/cavazquez/openttdrs/issues/599); Squirrel, nuevos
objetivos y ventanas de historia no forman parte de este corte.
