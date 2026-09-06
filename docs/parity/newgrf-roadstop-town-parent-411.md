# RoadStop parent TownScope — issue #411

Actualizado: 2026-09-06.

## Divergencia observada

En OpenTTD, `RoadStopScopeResolver::GetScope(VSG_SCOPE_PARENT)` devuelve un
`TownScopeResolver`. Antes de este corte, `openttdrs` podía calcular las
variables locales del `RoadStop` y las variables `0x45`/`0x46` de mundo, pero
dejaba vacío `parent_vars` y `parent_persistent_registers`. Un Action2 que
leyera población, flags, radios o `7C` del pueblo padre recibía por tanto el
fallback cero incluso cuando el renderer ya disponía del pool de pueblos.

## Cambio ejecutable

La ruta
`action2_eval_ctx_for_road_stop_tile_with_catalog_and_world` ahora selecciona
el pueblo más cercano mediante `(distancia Manhattan, town.id)`, la misma clave
determinista que usa el scope local `0x45`/`0x46`, y llama a
`Town::copy_newgrf_parent_scope` con el GRFID de la parada. Se copian las
variables de `TownScopeResolver` que el modelo conserva (`0x40`, `0x41`,
`0x80`–`0x83`, crecimiento, flags, radios, ratings, historial y contadores) y
el PSA persistente asociado a ese GRFID en `parent_persistent_registers`.

La selección por pueblo cercano es un fallback explícito: el modelo todavía no
guarda un puntero nativo `RoadStop`→`Town`. Las APIs legacy que no reciben
`RoadStopWorldContext` siguen dejando el parent vacío para no inventar una
entidad. Un mundo sin pueblos también conserva el contexto vacío.

## Regresión

`road_stop_world_scope_exposes_parent_town_and_psa_by_grfid` crea una parada
catalogada, un pueblo con `larger_town`, población y un registro PSA, y verifica
que el contexto map-aware devuelve las variables parent y el registro correcto
por GRFID. La prueba también conserva el aislamiento de las rutas legacy.

## Residual

Quedan pendientes la asociación nativa persistente entre cada parada y su
pueblo (si el SAV la aporta), variables de TownScope que el modelo aún no
representa y los scopes parent de las demás entidades `BaseStation`. El padre
[#329](https://github.com/cavazquez/openttdrs/issues/329) continúa abierto.
