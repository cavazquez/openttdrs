# Station cargo guards (`#405`)

Actualizado: 2026-09-06.

OpenTTD no trata todos los contadores de `GoodsEntry` como datos disponibles:

- `0x61` (`time_since_pickup`) devuelve cero mientras ningún vehículo haya
  intentado cargar (`last_speed == 0`).
- `0x63` (`PeriodsInTransit`) consulta la cola sólo si `HasData()` es cierto;
  una estación sin packets devuelve cero.

El contexto compartido por las rutas legacy y map-aware aplica ambas reglas.
La familia `0x64` sigue usando su sentinel `0xFF00` y los slots deprecated
`0x8C..0xEC` conservan sus reglas históricas, por lo que el cambio no mezcla
los contratos de `Station::GetNewGRFVariable`.

Regresión: `station_cargo_scope_honours_vehicle_and_data_guards` cubre una
estación con espera pero sin intento/datos y la misma estación después de
registrar un intento y un packet con cuatro períodos de tránsito.

Referencia upstream: `newgrf_station.cpp` y `station_base.h`
(`HasVehicleEverTriedLoading`, `HasData`).
