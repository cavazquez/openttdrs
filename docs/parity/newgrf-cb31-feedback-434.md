# CB31: motivo textual en el feedback de start/stop (#434)

Actualizado: 2026-09-06.

## Qué se cerró

Después de clasificar el resultado de `CBID_VEHICLE_START_STOP_CHECK` (#431) y
expandir referencias inline del catálogo (#433), el comando conserva un
diagnóstico efímero con `(vehicle_id, GRFID, outcome)` sólo cuando el callback
rechaza la operación. Los tres puntos de entrada visibles de start/stop (lista
de flota, ventana del vehículo y depósito) consumen ese diagnóstico y resuelven
el StringID mediante `NewGrfStringCatalog::lookup_expanded` con el locale activo.

Una cadena Action4/Action13 anidada aparece ahora como motivo en el HUD. El
diagnóstico se elimina después de la lectura, no forma parte de JSON/SAV y se
descarta si pertenece a otro vehículo.

## Fallback deliberado

Los resultados `GenericDenied`, las cadenas ausentes/vacías y cualquier error
que no sea CB31 mantienen el mensaje seguro existente. El resultado del comando
no cambia y el callback no se ejecuta una segunda vez para construir el texto.

## Alcance pendiente

La localización completa de errores de construcción y los parámetros dinámicos
del text stack todavía requieren el catálogo de mensajes upstream y scopes de
`TextRefStack`; este corte sólo hace observable el motivo ya disponible de CB31.
