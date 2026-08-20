# Zoom máximo y etiquetas del mapa

La captura candidata se tomó con `Kale_TitleGame.sav`, 1832×960, centro
`128,128`, OpenGFX 8bpp, UI visible y `OPENTTDRS_MAP_SHOT_SCALE=8`.

El valor solicitado se acota dentro del cliente por `clamp_ortho_scale`: para
1832×960 el presupuesto de spawn (`MAX_SPAWN_SPAN_TILES=192`) deja una escala
máxima aproximada de 2,66. Por eso el modo fijo sólo alcanza la entrada
equivalente a `Out2x`; OpenTTD mantiene los seis niveles hasta `Out8x`.

La imagen original es la captura aportada por el usuario en la solicitud del
issue (OpenTTD 15.3, `Out8x`, con labels visibles). El cliente no expone esa
imagen del chat como un archivo local reutilizable; el issue conserva la
descripción y la candidata versionada para que la referencia se adjunte allí
sin inventar una captura distinta.
