# Station parent TownScope — issue #412

Actualizado: 2026-09-06.

## Divergencia observada

`StationResolverObject::GetScope(VSG_SCOPE_PARENT)` de OpenTTD devuelve un
`TownScopeResolver`. La ruta de estación de `openttdrs` ya podía resolver las
variables propias y el catchment con pools de mundo, pero el contexto Action2
no cargaba `parent_vars` ni `parent_persistent_registers`. Los Action2 de una
estación colocada que consultaran población, flags o `7C` veían el fallback
cero.

## Cambio ejecutable

`StationAction2WorldContext` recibe ahora el pool de pueblos. Los contextos
catalog-aware seleccionan el pueblo más cercano mediante
`(distancia Manhattan, town.id)`, igual que los fallbacks de mundo existentes,
y llaman a `Town::copy_newgrf_parent_scope` con el GRFID de la
`StationSpecDef`. Se copian las variables de TownScope modeladas y el storage
persistente `7C` correcto por GRFID.

El renderer de estaciones, la construcción de estaciones/waypoints y los
wrappers de animación/scheduler CB140–142 que reciben pools usan variantes que
propagan `GameState::towns`. Las APIs públicas históricas sin catálogo o sin
pueblos conservan parent vacío para no inventar una asociación.

## Regresión y gates

`station_world_scope_exposes_parent_town_and_psa_by_grfid` crea una estación
catalogada y un pueblo con población, flag `larger_town` y un registro PSA; el
contexto verifica las variables parent y el registro seleccionado por GRFID.
Los wrappers de animación se compilan con el mismo `StationAction2WorldContext`
y pasan el pool real desde los call sites del scheduler.

## Residual

El modelo todavía no conserva el vínculo nativo persistente estación→pueblo ni
todas las variables/strings/sonidos de `BaseStation`. Esas brechas continúan
en [#329](https://github.com/cavazquez/openttdrs/issues/329).
