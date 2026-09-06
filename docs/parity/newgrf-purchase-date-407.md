# Fecha de compra NewGRF en Station/RoadStop (#407)

Actualizado: **2026-09-06**.

## Divergencia encontrada

El reloj del core guarda `CalendarTimer.date` como cantidad de días desde el
1 de enero de 1950. En cambio, `OpenTTD` conserva `TimerGameCalendar::date` en
la escala absoluta que usa `CalendarTime`, y los scopes de estación/parada
calculan `0xFA` restando `DAYS_TILL_ORIGINAL_BASE_YEAR` y saturando a WORD.

El helper de compra ferroviaria ya hacía la resta correcta, pero el preflight
de `PlaceRailStation`/`PlaceRailStationArea` le pasaba sólo el día relativo.
El picker de RoadStop tampoco publicaba `0xFA`.

## Corrección

- Los dos preflights pasan `STATION_BUILD_DATE_DEFAULT + calendar.date`.
- El scope contextual de RoadStop publica `0xFA` con la misma resta y
  saturación que `StationScopeResolver`.
- Los wrappers legacy siguen usando la fecha base y producen `0` de forma
  determinista cuando no tienen reloj.

## Regresiones

Los tests de comando construyen una spec NewGRF cuyo callback devuelve `0xFA`:

- `place_rail_station_availability_sees_absolute_calendar_date`
- `place_bus_stop_availability_sees_absolute_calendar_date`

Ambos fijan el día 123 y sólo permiten la construcción si el callback recibe
la fecha absoluta y expone `123` tras la conversión. La regresión contextual de
RoadStop también comprueba la saturación y el callback directo.

## Estado

La corrección es independiente del storage `7C` y no crea entidades
temporales durante la compra. Los scopes de vecinos, strings y sonidos siguen
pendientes en [#329](https://github.com/cavazquez/openttdrs/issues/329).
