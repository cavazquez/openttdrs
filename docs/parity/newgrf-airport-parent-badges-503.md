# Badges del scope padre de `AirportTile` NewGRF (#503)

Actualizado el 2026-09-07.

## Divergencia corregida

`AirportTile` ya exponía `Action2 var 0x7A[param]` para los badges propios de
la tesela. Cuando un grupo variacional usaba el scope padre (`0x82`, `0x86` o
`0x8A`), sin embargo, el contexto no materializaba
`parent_parameterized_vars[(0x7A, param)]`. La selección caía en la rama
default, aunque el aeropuerto padre tuviera el badge pedido. La clave de caché
también ignoraba esos valores parametrizados del padre, con riesgo de reutilizar
el sprite de un aeropuerto en otro que compartiera el mismo `AirportTile` gfx.

## Oráculo nativo

- `src/newgrf_airporttiles.cpp` construye `AirportTileResolverObject` con el
  GRF del `AirportTile` y un `AirportScopeResolver` como padre.
- `src/newgrf_airport.cpp`, `AirportScopeResolver::GetVariable(0x7A)`, llama
  `GetBadgeVariableResult(*ro.grffile, spec->badges, parameter)`.

Por tanto, el índice local se traduce mediante `GlobalVar 0x18` del GRF que
está ejecutando el tile, mientras la presencia se comprueba contra la lista de
badges del `AirportSpec` construido. No se usa la tabla de traducción del GRF
del aeropuerto padre para interpretar una expresión del tile.

## Contrato implementado

- `action2_eval_ctx_for_airport_tile_with_towns_and_airport_catalog` separa
  ambas fuentes y sólo materializa `0x7A` del padre cuando el grafo Action2 lo
  solicita. Un aeropuerto vanilla sin badges devuelve `0`; un spec NewGRF
  ausente o una entrada de traducción inválida devuelve `UINT_MAX`.
- El renderer, `CB150` y las rutas de `CB152`/`CB153`/`CB154` de eventos y
  scheduler reciben `airport_spec_catalog`, de modo que la imagen y los
  callbacks comparten el mismo scope. Las APIs históricas sin ese catálogo
  permanecen como fallback explícito y no inventan un padre NewGRF.
- `runtime_fingerprint` mezcla ordenadamente `parent_parameterized_vars` con
  una marca propia, evitando colisiones con parámetros del scope de la tesela.

Las regresiones
`airport_tile_parent_badge_selects_the_parent_scope_action2_branch`,
`built_newgrf_airport_uses_parent_badge_action2_sprite` y
`parent_and_relative_scopes_change_fingerprint` cubren la selección de rama
core, el camino ECS real y la invalidación de caché respectivamente.

## Límites que continúan abiertos

Esto cubre sólo `0x7A` del padre. La FTA propia, paletas y sprites base de
`TileLayout`, foundations Action5/rotaciones, sonidos y el resto del
`AirportScopeResolver` continúan abiertos en #326/#329.
