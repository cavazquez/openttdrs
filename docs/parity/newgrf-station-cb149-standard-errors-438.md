# CB149: errores estándar localizados (#438)

Actualizado: 2026-09-06.

El feedback de construcción ferroviaria traduce ahora los códigos estándar que
OpenTTD 15.3 devuelve desde `GetErrorMessageFromLocationCallbackResult`:

| Código | Español | Inglés |
| --- | --- | --- |
| `0x402` | Sólo se puede construir en selva. | This can only be built in rainforest. |
| `0x403` | Sólo se puede construir en desierto. | This can only be built in desert. |
| `0x404` | Sólo se puede construir por encima de la línea de nieve. | This can only be built above the snow line. |
| `0x405` | Sólo se puede construir por debajo de la línea de nieve. | This can only be built below the snow line. |
| `0x406` | No se puede construir en el mar. | This cannot be built on sea. |
| `0x407` | No se puede construir sobre un canal. | This cannot be built on a canal. |
| `0x408` | No se puede construir sobre un río. | This cannot be built on a river. |

`0x401` y resultados desconocidos mantienen el fallback genérico. La tabla se
aplica sólo al diagnóstico CB149 ya validado; no cambia la aceptación de la
pendiente ni persiste texto en el estado.
