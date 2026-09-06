# CB149: scope de compra de estaciones

Actualizado: 2026-09-06 · issue [#441](https://github.com/cavazquez/openttdrs/issues/441)

La ruta `PlaceRailStation` evalúa CB149 antes de crear la estación, igual que
`PerformStationTileSlopeCheck` de OpenTTD 15.3. Además de `param1`, `param2` y
la información de terreno `0x67[param]`, el resolver map-aware ahora
materializa el scope `st == nullptr` de compra:

- `0x40`, `0x41`, `0x46`, `0x47` y `0x49` devuelven el sentinel
  `0x02110000` de plataforma/posición sin tesela.
- `0x42` devuelve el tipo de vía de compra (cero en este corte), `0x44` el
  estado PBS de compra (`2`) y `0x43` el `GetCompanyInfo` de la compañía que
  emite el comando.
- `0x7A[param]` usa la tabla de badges de la `StationSpecDef` y `0xFA` la fecha
  relativa de calendario saturada a WORD.

La API legacy sin mapa mantiene su contrato. La API map-aware anterior usa
valores vanilla explícitos; el call site de construcción pasa ahora compañía,
pool, color y fecha reales para que query/preview y execute seleccionen la
misma rama Action2. Las variables que requieren una estación creada (por
ejemplo continuidad/vecinos `0x45`, `0x68`, `0x6A` y `0x6B`) siguen fuera de
este preflight y se resuelven en los contextos de render/animación.
