# Zoom máximo y etiquetas del mapa

## Estado tras la corrección de zoom máximo

Cuando el viewport materializado no supera 512×512 teselas, el cliente conserva
el render detallado en `Out4x` y `Out8x`, igual que OpenTTD: siguen presentes
rutas, vías, edificios, puentes, vehículos y el terreno completo. Es un
presupuesto del recorte visible, no del mapa entero; por eso también preserva
detalle al alejar un mapa de 512×512 o mayor si ese recorte cabe en el límite.
La captura de `Kale_TitleGame.sav` se ejecuta por este camino; antes de la
corrección se sustituía por rombos agregados y quedaban grietas negras entre
bloques.

Para viewports que superan ese presupuesto se mantiene un camino de protección:
`Out4x` resume bloques de 4×4 y `Out8x` bloques de 8×8 por mayoría de teselas
(agua tiene prioridad en empate, bosque exige mayoría estricta) y altura media.
Ese modo evita crear cientos de miles de entidades al encuadrar una región
demasiado amplia, pero todavía omite las capas detalladas de infraestructura y
vehículos; no se debe presentar como paridad raster completa. Cada bloque usa
el rombo lógico de 64×32 píxeles y un respaldo opaco del color del terreno: así
las esquinas transparentes del sprite OpenGFX no abren grietas negras entre
bloques ampliados.

Las etiquetas compensan la escala ortográfica para seguir siendo legibles y se
componen en el mismo orden de OpenTTD: pueblos → carteles → estaciones. No se
eliminan por colisión; el viewport oficial agrega todos los signos dentro del
rectángulo y por eso pueden superponerse densamente en `Out8x`, como en la
captura de referencia. Todavía falta validar rastermente la captura 1832×960
contra OpenTTD y portar la selección espacial/ownership exacta del índice de
labels de OpenTTD.

La captura candidata se tomó con `Kale_TitleGame.sav`, 1832×960, centro
`128,128`, OpenGFX 8bpp, UI visible y `OPENTTDRS_MAP_SHOT_SCALE=8`.

Antes del port, el valor solicitado se acotaba dentro del cliente por
`clamp_ortho_scale`: para 1832×960 el presupuesto de spawn
(`MAX_SPAWN_SPAN_TILES=192`) dejaba una escala máxima aproximada de 2,66 y el
modo fijo sólo alcanzaba `Out2x`. OpenTTD mantiene los seis niveles hasta
`Out8x`; el render detallado para viewports dentro del presupuesto y el camino
agregado para recortes mayores eliminan ese límite sin volver a materializar el
mapa completo de 4096×4096.

## Comprobación reproducible

La matriz aleatoria de 1024×1024 con semilla `1331024978` conserva el contrato
`world-raw` tesela a tesela frente al generador de OpenTTD: `tiles=0` y
`blocks4=0/65536`. Sobre ese mismo `.sav`, las capturas limpias a 1280×720 en
`Out1x`, `Out2x`, `Out4x` y `Out8x`, y la captura con UI/labels a 1832×960 en
`Out8x`, conservaron sprites detallados y no mostraron huecos diagonales. Esto
verifica el camino del cliente; no sustituye la comparación raster directa con
la captura de OpenTTD que sigue pendiente en el issue.

La imagen original es la captura aportada por el usuario en la solicitud del
issue (OpenTTD 15.3, `Out8x`, con labels visibles). El cliente no expone esa
imagen del chat como un archivo local reutilizable; el issue conserva la
descripción y la candidata versionada para que la referencia se adjunte allí
sin inventar una captura distinta.
