# V1-NET — late join TCP de dos clientes

El contrato [#595](https://github.com/cavazquez/openttdrs/issues/595)
certifica una sesión finita sobre el transporte propio. No declara
interoperabilidad con OpenTTD ni cobertura de Internet adversarial.

## Fixture, autoridad y log

`crates/openttdrs-net/tests/v1_late_join.rs` levanta un `ListenServer`
headless —la misma autoridad TCP que utiliza `openttdrs-dedicated`— y dos
clientes reales sobre `127.0.0.1`. Parte de `first_route`: Temperate plano de
64×64, mina de carbón `(8,8)`, central `(48,8)`, carretera, depósito, dos
paradas, camión vanilla de carbón y órdenes `carga completa → descarga`.

El primer cliente entra antes del primer avance y emite `PlaceRoad(2,50)` como
su compañía asignada. El segundo entra después del avance de sesión 500,
recibe el snapshot de esa frontera y emite `PlaceRoad(3,50)` con su compañía
distinta. El log servidor esperado queda limitado a esos dos `Commit` con
`seq` 1 y 2; el late join sólo debe recibir el sufijo desde el `next_seq` de su
`Welcome`, para no reaplicar lo que el snapshot ya contenía.

Los peers ya conectados consumen cada `AdvanceTicks(1)` por TCP. El listener
conserva su copia autoritativa durante esos avances y materializa el snapshot
en la barrera FIFO del join; el contrato publica explícitamente el snapshot
del host en el tick 500 antes de abrir el segundo cliente. Esto evita trabajo
de serialización repetido, pero no reduce los 2.000 avances ni las
comparaciones de hash y log por tick.

## Aceptación reproducible

```sh
OPENTTDRS_SOURCE_SHA="$(git rev-parse HEAD)" \
  cargo test --locked --offline -p openttdrs-net --test v1_late_join -- --nocapture
```

La prueba avanza exactamente 2.000 `AdvanceTicks(1)`. Después de cada avance,
compara `canonical_hash()` del dedicated y del cliente inicial; desde el join
en el tick 500 compara también el late join. En cada tick verifica que cada
log de cliente sea exactamente el sufijo del log del servidor que admite su
`next_seq`: ningún comando puede perderse ni duplicarse. También verifica los
dos issuers, compañías exclusivas y slots materializados en los tres estados.

El transporte no se simula: `PermissionDenied` al crear listener o conectar
cliente produce `panic`. El plazo global de la certificación es 120 s; los
sondeos loopback usan 1 ms sólo para acelerar la entrega local, sin ampliar
ese límite. Al final se envía un `HashCheck` real del protocolo además de las
comparaciones por tick.

La salida emite un JSON `V1-NET late join report` con `source_sha`, ticks
solicitados/observados, clientes, tick del join, log de comandos, primer
desacuerdo (o `null`) y hash final. `OPENTTDRS_SOURCE_SHA` liga la evidencia
local al candidato; GitHub Actions usa `GITHUB_SHA` automáticamente.

El caso también cubre dos reparaciones necesarias para que el snapshot sea
autoridad reproducible: una compañía materializada por red nace con el
`max_loan` global vigente y `hydrate_runtime` restituye el `sim_tick` efímero
de cada vehículo al tick del snapshot antes de la siguiente fase de órdenes.

## Regresiones conservadas

El contrato no sustituye los recorridos existentes de frontera/resync y host
migration; se ejecutan sin cambiar su alcance:

```sh
cargo test --locked --offline -p openttdrs-net --test tcp_lockstep \
  snapshot_frontier_applies_commits_and_advances_exactly_once -- --nocapture
cargo test --locked --offline -p openttdrs-net --test host_migration -- --nocapture
```

Quedan fuera lobbies, autenticación/cifrado, más de dos clientes, host
migration adicional, red hostil y protocolo multijugador de OpenTTD. El cierre
de #595 requiere CI remota verde para el SHA publicado que contiene este
contrato.
