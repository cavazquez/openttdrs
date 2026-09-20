# V1-SHIP — primera entrega marítima de carbón

El contrato [#593](https://github.com/cavazquez/openttdrs/issues/593) certifica
un viaje naval pequeño y reproducible. No declara paridad general de barcos.

## Fixture y log público

El test `crates/openttdrs-core/tests/v1_ship_delivery.rs` parte del fixture
Temperate `first_route`: mapa plano de 64×64, semilla `0xC0A1_1950`, año 1950,
mina de carbón en `(8,8)` y central eléctrica en `(48,8)`. La franja fija de
mar es `x=4..57`, `y=12..14`; sólo forma parte del mapa inicial y no introduce
vehículos, carga ni dinero.

El log público construye, en este orden:

1. depósito naval `(5,13)`, dirección `2`;
2. muelle de carga `(12,11)`, dirección `1`;
3. muelle de descarga `(52,11)`, dirección `1`;
4. boya `(32,13)`;
5. un `MPS Coal Trader` vanilla;
6. órdenes `carga completa → waypoint boya → descarga`;
7. inicio del barco.

La boya usa `VehicleOrder::waypoint`: no es una estación de carga. La corrección
de `finish_arrival_after_load_window_with_catalog` limita la espera por descarga
a órdenes `Station`, para que un barco cargado no quede detenido sobre una boya.

## Aceptación reproducible

```bash
cargo test --locked --offline -p openttdrs-core --test v1_ship_delivery -- --nocapture
```

La corrida fija observada entrega 4 unidades de carbón en el tick 4.101, después
de pasar la boya en el tick 2.970 con 100 unidades a bordo. El ingreso acumulado
es 146 y el hash canónico tras la primera entrega es
`1496686749879025395`.

El test ejecuta el mismo log dos veces y exige igualdad de tick de boya, carga a
bordo, tick/unidades/ingreso de la primera entrega y hash final. Además verifica:

- entrega física aceptada por la central y con ingreso positivo antes de 40.000
  ticks;
- conservación `stock inicial + producción = espera + a bordo + entrega final`;
- acreditación de carbón por tipo a la compañía activa;
- la boya no recibe ni descarga carbón;
- las industrias, muelles y vehículo se crean por el fixture/log declarado, sin
  inyección de vehículo, carga o ingreso durante la operación.

## Límite explícito

No cubre canales, esclusas, reservas, YAPF naval global, varias naves, NewGRF
navales ni una comparación física externa con OpenTTD. Es un único servicio de
carbón por mar con una boya y dos muelles.
