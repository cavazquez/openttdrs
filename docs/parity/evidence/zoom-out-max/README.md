# Zoom máximo y etiquetas del mapa

## Estado tras el port de overview

Desde el port de overview el cliente ya no recorta los niveles fijos por el presupuesto
de culling: `Out4x` usa bloques agregados de 4×4 y `Out8x` bloques de 8×8.
Esto mantiene el mapa cubierto sin instanciar cada sprite de detalle. En esos
niveles los vehículos y las capas de infraestructura detalladas se omiten a
propósito; el color del bloque conserva la lectura macro de terreno, agua y
redes.

Las etiquetas compensan la escala ortográfica para seguir siendo legibles y
aplican colisión determinista con prioridad pueblos → carteles → estaciones.
Todavía falta validar rastermente la captura 1832×960 contra OpenTTD y portar
la selección espacial/ownership exacta del índice de labels de OpenTTD.

La captura candidata se tomó con `Kale_TitleGame.sav`, 1832×960, centro
`128,128`, OpenGFX 8bpp, UI visible y `OPENTTDRS_MAP_SHOT_SCALE=8`.

Antes del port, el valor solicitado se acotaba dentro del cliente por
`clamp_ortho_scale`: para 1832×960 el presupuesto de spawn
(`MAX_SPAWN_SPAN_TILES=192`) dejaba una escala máxima aproximada de 2,66 y el
modo fijo sólo alcanzaba `Out2x`. OpenTTD mantiene los seis niveles hasta
`Out8x`; el camino agregado descrito arriba elimina ese límite de escala sin
convertir el overview en un spawn masivo.

La imagen original es la captura aportada por el usuario en la solicitud del
issue (OpenTTD 15.3, `Out8x`, con labels visibles). El cliente no expone esa
imagen del chat como un archivo local reutilizable; el issue conserva la
descripción y la candidata versionada para que la referencia se adjunte allí
sin inventar una captura distinta.
