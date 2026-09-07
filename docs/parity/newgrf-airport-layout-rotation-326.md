# Selector, layout y rotación geométrica de aeropuertos NewGRF (#326 / #328)

Actualizado el 2026-09-07.

## Divergencia corregida

La construcción exponía sólo el selector compacto X/Y. No permitía elegir un
índice Action0 concreto cuando un aeropuerto NewGRF declaraba N/E/S/O (u otros
layouts), y validaba el rectángulo completo de `size_x × size_y`. Eso podía
rechazar o limpiar una tesela ocupada dentro de un hueco que Action0 no
declara. Además, la ruta anterior podía transponer offsets para el eje Y: las
teselas y sus `AirportTile` gfx se construían en coordenadas distintas de las
de Action0, mientras `STNN.normal.airport.layout` y `rotation` describían otra
variante. Al reabrir el SAV, la rehidratación repetía la transposición.

## Oráculo nativo

En OpenTTD 15.3 y el upstream fijado:

- `newgrf/newgrf_act0_airports.cpp`, propiedad `0x0A`, normaliza cada
  `layout.rotation` con `& 6` y calcula el tamaño canónico intercambiando
  dimensiones para E/O.
- `airport_gui.cpp` conserva `_selected_airport_layout`, lo reinicia al elegir
  un aeropuerto, lo acota al catálogo y lo mueve con los botones anterior y
  siguiente. Ese índice llega sin reducción a `CmdBuildAirport`.
- `station_cmd.cpp`, `CmdBuildAirport`, rechaza un layout inexistente,
  intercambia únicamente `size_x/size_y` para E/O y entrega el índice elegido a
  `AirportTileTableIterator`.
- `newgrf_airport.h`, `AirportTileTableIterator`, suma los offsets del layout
  directamente al origen. No rota ni transpone `(x,y)` al iterarlos.
- `station_cmd.cpp`, `CheckFlatLandAirport`, itera ese mismo iterador: sólo
  comprueba, despeja y cobra las teselas Action0 declaradas; el rectángulo se
  reserva para la geometría de estación/selección, no para convertir huecos en
  suelo obligatorio.

## Contrato implementado

- El picker lista los aeropuertos NewGRF habilitados del catálogo Action0 junto
  a los vanilla. Al elegir uno inicializa el índice 0 y muestra los botones
  anterior/siguiente y `Layout n/N · dirección`; el clic derecho también
  recorre los índices exactos. Su altura se calcula desde el contenido para que
  catálogo, orientación y cobertura no queden fuera del marco. X/Y conserva un
  atajo compatible que prefiere N/E y luego S/O del mismo eje.
- El comando nuevo `PlaceAirportAreaWithLayout` lleva el id global del spec y
  el índice Action0. La previsualización, la consulta y el execute usan la
  misma pareja, por lo que una selección local posterior no puede cambiar una
  acción ya emitida; un id o índice ausente devuelve `InvalidAirportLayout`.
- La huella intercambia sólo sus dimensiones cuando el layout seleccionado es
  E/O. Las coordenadas declaradas se materializan sin transformación.
- Para layouts NewGRF la comprobación de terreno, ocupación y nivel recorre
  exclusivamente las coordenadas declaradas por Action0, empezando el nivel
  plano en la primera de ellas. Un hueco interior no se valida ni se limpia.
- La construcción persiste el índice y la rotación del layout realmente usado
  en `STNN.normal.airport.layout`/`rotation`, junto con los gfx por tesela.
- La rehidratación SAV usa ese selector exacto y busca el origen con los
  offsets directos, por lo que no puede reatachar una variante transpuesta.

Las regresiones
`newgrf_airport_build_uses_declared_east_layout_without_transposing_tiles`,
`explicit_newgrf_layout_uses_only_declared_tiles_and_rejects_unknown_index` y
`rehydrates_east_layout_coordinates_without_transposing_them` comprueban una
variante E cuyo segundo gfx y posición no pueden obtenerse transponiendo la
variante N, y un layout disperso cuyo hueco contiene vía ocupada. La primera
también verifica la huella 2×4 y el selector persistido; la segunda prueba que
el comando no depende de la selección actual. En cliente,
`newgrf_picker_selects_and_cycles_the_exact_action0_layout` y
`airport_command_keeps_the_explicit_newgrf_layout` fijan la elección y el
transporte del índice al comando.

## Límites que continúan abiertos

Esto resuelve la selección y geometría Action0 de construcción, no la
presentación completa: permanecen foundations de compositor, rotación runtime
de sprites/children, paletas, sonidos, callbacks de nombres/texto de layout y
la FTA propia de aeropuertos NewGRF. Por eso #326, #328 y #329 siguen abiertos;
esta nota no afirma paridad raster ni interoperabilidad SAV global.
