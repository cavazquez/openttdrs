# NewGRF Station StringID/BuildDate — corte #400

Actualizado: 2026-09-06.

## Alcance

Este corte cubre las dos variables de `StationScope` que identifican la
plantilla de nombre y la fecha de construcción de una `BaseStation`:

- `0x84` devuelve `BaseStation::string_id` como `StringID` de 32 bits, con el
  valor nativo `STR_SV_STNAME` (`0x6006`) por defecto y
  `STR_SV_STNAME_FALLBACK` (`0x6027`) para una estación renombrada cuando el
  save antiguo no conserva el campo.
- `0xFA` devuelve `build_date - DAYS_TILL_ORIGINAL_BASE_YEAR`, saturado a
  `uint16`, igual que `GetStationVariable` de OpenTTD 15.3.

## Persistencia y creación

`Station` conserva ambos valores con defaults compatibles con estados JSON
anteriores. El lector SAV obtiene `string_id` y `build_date` de `STNN.base`
(filas modernas y legacy), los hidrata al importar y el writer los vuelve a
emitir sin reemplazarlos por constantes. Las estaciones creadas por los
comandos de tren, carretera, waypoint, muelle, boya y aeropuerto reciben la
fecha absoluta del calendario actual. Renombrar actualiza la plantilla
fallback; el texto visible sigue siendo el nombre custom del modelo.

## Regresiones

- contexto legacy de Action2: `0x84` y `0xFA` con valores custom;
- round-trip JSON de `Station`;
- writer → parser de `STNN` con `StringID` y fecha no nativos;
- compatibilidad de fixtures y constructores sintéticos que omiten los nuevos
  campos.

## Fuera de alcance

La resolución completa de strings (town/company parameters, idioma y
renderizado), los callbacks de construcción sin una estación existente y el
resto de scopes de `BaseStation`/aeropuerto siguen pendientes en #329.
