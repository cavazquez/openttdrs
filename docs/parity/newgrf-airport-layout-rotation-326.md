# Layout y rotación geométrica de aeropuertos NewGRF (#326 / #328)

Actualizado el 2026-09-07.

## Divergencia corregida

La construcción con el selector compacto X/Y elegía antes un layout norte/sur
por defecto y, para el eje Y, transponía sus offsets. Eso podía construir las
teselas y sus `AirportTile` gfx en coordenadas distintas de las que declara
Action0, mientras `STNN.normal.airport.layout` y `rotation` describían otra
variante. Al reabrir el SAV, la rehidratación repetía la transposición.

## Oráculo nativo

En OpenTTD 15.3:

- `newgrf/newgrf_act0_airports.cpp`, propiedad `0x0A`, normaliza cada
  `layout.rotation` con `& 6` y calcula el tamaño canónico intercambiando
  dimensiones para E/O.
- `station_cmd.cpp`, `CmdBuildAirport`, intercambia únicamente `size_x/size_y`
  para E/O y entrega el layout seleccionado a `AirportTileTableIterator`.
- `newgrf_airport.h`, `AirportTileTableIterator`, suma los offsets del layout
  directamente al origen. No rota ni transpone `(x,y)` al iterarlos.

## Contrato implementado

- El selector X/Y prefiere N/E; si falta esa dirección usa S/O del mismo eje y,
  como último fallback, el primer layout declarado. La UI aún no expone una
  elección explícita de las cuatro direcciones.
- La huella intercambia sólo sus dimensiones cuando el layout seleccionado es
  E/O. Las coordenadas declaradas se materializan sin transformación.
- La construcción persiste el índice y la rotación del layout realmente usado
  en `STNN.normal.airport.layout`/`rotation`, junto con los gfx por tesela.
- La rehidratación SAV usa ese selector exacto y busca el origen con los
  offsets directos, por lo que no puede reatachar una variante transpuesta.

Las regresiones
`newgrf_airport_build_uses_declared_east_layout_without_transposing_tiles` y
`rehydrates_east_layout_coordinates_without_transposing_them` comprueban una
variante E cuyo segundo gfx y posición no pueden obtenerse transponiendo la
variante N. La primera también verifica la huella 2×4 y el selector persistido.

## Límites que continúan abiertos

Esto resuelve la geometría Action0 y la persistencia, no la presentación
completa: permanecen Action5/foundations de compositor, rotación runtime de
sprites/children, paletas, sonidos y la FTA propia de aeropuertos NewGRF. Por
eso #326, #328 y #329 siguen abiertos; esta nota no afirma paridad raster ni
interoperabilidad SAV global.
