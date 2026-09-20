# V1-EDIT — escenario editado, guardado y abierto desde el menú

El contrato [#600](https://github.com/cavazquez/openttdrs/issues/600) certifica
un único escenario propio Temperate de 64×64. No declara compatibilidad con
`.scn` universal, heightmaps, editores NewGRF ni todos los tamaños o climas.

## Recorrido productivo

`editor_v1_roundtrip_via_menu_preserves_edited_scenario` crea el mundo del
editor con sus settings públicos, activa el sandbox y fija la fecha en 1975.
Por el dispatcher de comandos que usa la UI eleva una tesela, funda un pueblo
y coloca una mina de carbón y una central eléctrica. La prueba conserva las
posiciones finales reales, tipos, layouts, mapa, clima, semilla, settings,
fecha, conteos y hash canónico.

El recorrido abre el guardado mediante `OpenUiRoute::EditorSaveScenario` y
confirma el modal real. Primero apunta la carpeta de escenarios a un archivo
regular temporal: el error de `create_dir_all` mantiene el documento marcado
como modificado y la ventana abierta. Luego guarda `editor-v1.json` en un
`tempdir` aislado; el sufijo explícito exige el formato JSON completo del
editor.

Después envía `EditorExit`, verifica la transición solicitada al menú, pulsa
los botones productivos **Escenarios** y **Abrir carpeta de escenarios**,
selecciona el archivo y confirma la carga en la misma ventana. La carga usa
`apply_loaded_state` desde el manejador de producción; la prueba no asigna el
mundo reabierto. Comprueba además que el tick fijado antes de editar permanece
exactamente igual: guardar, salir y abrir no ejecutan la simulación.

## Ejecución

Desde la raíz del repositorio:

```sh
cargo test --locked --offline -p openttdrs-client \
  editor_v1_roundtrip_via_menu_preserves_edited_scenario -- --nocapture
```

La aserción final exige igualdad del hash canónico y de las listas completas de
pueblos e industrias relevantes, además de los conteos originales más uno y
más dos respectivamente. Así cualquier entidad perdida, duplicada o cambiada
en el round-trip falla el contrato.

## Límite explícito

No cubre el formato `.scn` de OpenTTD, reabrir heightmaps, múltiples mapas,
edición NewGRF, UI visual ni continuidad de una simulación ya en marcha. El
cierre de #600 requiere también CI remota verde para el SHA publicado que
contenga esta prueba.
