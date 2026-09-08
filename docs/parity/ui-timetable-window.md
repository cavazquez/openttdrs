# Localización de la ventana de horario

Actualizado: 2026-09-08. Sub-issue [#534](https://github.com/cavazquez/openttdrs/issues/534)
de [#331](https://github.com/cavazquez/openttdrs/issues/331).

## Diferencia y contrato del corte

Aunque el jugador seleccionaba inglés, `sync_timetable_window` escribía el
título, el resumen de retraso y las filas de órdenes siempre en español.
Los botones `Horario ON/OFF`, `Poner en hora`, `Ticks/Seg` y `Viaje` tampoco
tenían traducción. El constructor creaba además un título estático que podía
quedar registrado en el catálogo antes de incorporar el nombre del vehículo.

La referencia nativa es `src/timetable_gui.cpp` y los catálogos
[`src/lang/english.txt`](https://github.com/OpenTTD/OpenTTD/blob/15.3/src/lang/english.txt)
y [`src/lang/spanish.txt`](https://github.com/OpenTTD/OpenTTD/blob/15.3/src/lang/spanish.txt):
`STR_TIMETABLE_TITLE`, `STR_TIMETABLE_STATUS_ON_TIME`,
`STR_TIMETABLE_STATUS_LATE`, `STR_TIMETABLE_STATUS_EARLY`,
`STR_TIMETABLE_TRAVEL_FOR`, `STR_TIMETABLE_STAY_FOR`,
`STR_TIMETABLE_RESET_LATENESS` y `STR_TIMETABLE_AUTOFILL` establecen que estas
superficies se muestran en el idioma activo.

El sincronizador consulta ahora `ClientPreferences::locale()` para ambos
slots de la cadena de vehículo. No mantiene una caché de texto: aplica idioma
y tiempos actuales en cada actualización y al reabrir. El título inicial
vacío deja su contenido dinámico bajo el control de ese sincronizador. El
catálogo existente traduce los controles estáticos; `Autorrelleno` es la
etiqueta española de `Autofill`.

El contrato preserva nombres personalizados, IDs, órdenes, foco, valores y
signos de retraso, formato de ticks/segundos y comandos. Las filas cubiertas
son estación, depósito, waypoint, condicional y destino de tesela.

El adaptador de captura individual `Timetable` conserva ocultos los slots sin
vehículo: antes forzaba visible también una segunda ventana vacía, que tapaba
las filas auditadas. Consulta `TimetableWindowState.slots`, donde `Some(0)` es
un vehículo válido; no interpreta `WindowKey.instance == 0` como ausencia.
Este filtro sólo afecta la captura individual de horarios y no cambia el
runtime normal, otras clases ni las capturas globales.

## Regresión ejecutable

```bash
cargo test -p openttdrs-client --bin openttdrs-client timetable_ --jobs 2
```

Resultado focalizado del 2026-09-08: **5 tests aprobados, 0 fallidos**.

`timetable_labels_cover_order_types_and_signed_lateness` cubre los cinco
tipos de orden, ticks/segundos, puntualidad y retrasos positivos/negativos,
incluido `i32::MIN` sin tomar un valor absoluto que pueda desbordar.

`timetable_windows_follow_locale_in_both_slots_without_mutating_vehicles`
construye las ventanas reales en una aplicación Bevy mínima y ejecuta el
plugin de localización junto con el sincronizador. Recorre `es → en → es`
durante varios frames, compara títulos, controles, resumen y filas de dos
vehículos, y verifica que se conserven foco, claves de ventana y todos los
datos serializados de ambos vehículos. Este test también inicializa las
queries ECS reales para detectar conflictos de componentes como `B0001`.
La prueba `timetable_shot_hides_only_unpopulated_individual_slots` cubre el
adaptador con ID cero, segundo slot vacío, dos slots poblados, marcador
ausente/fuera de rango y preservación de las demás modalidades de captura.

## Reproducción visual

Sobre un display funcional o Weston headless se puede capturar cada idioma
sin modificar las preferencias persistidas:

```bash
OPENTTDRS_SAV_LOAD=crates/openttdrs-core/tests/fixtures/mvp_openttd_rich.sav \
OPENTTDRS_WINDOW_SHOT_ID=Timetable \
OPENTTDRS_SHOT_RES=1280x720 \
OPENTTDRS_SHOT_UI_SCALE=1 \
OPENTTDRS_MAP_SHOT_SCALE=1 \
OPENTTDRS_LANGUAGE=en \
OPENTTDRS_WINDOWS_SHOT=/tmp/openttdrs-timetable-en.png \
cargo run -p openttdrs-client --features dynamic_linking
```

Repetir con `OPENTTDRS_LANGUAGE=es` y otro destino de captura. Para auditar la
misma ventana sobre varios niveles de zoom, repetir con
`OPENTTDRS_MAP_SHOT_SCALE=1`, `2`, `4` y `8`: equivalen a `1×`, `0.50×`,
`0.25×` y `0.125×`. El ID aceptado es `Timetable`; no existe un ID
`TimetableWindow`. El script
`scripts/capture_window_visual_baselines.sh` contiene el arranque Weston y
el adaptador de referencia OpenTTD para esa ventana.

El modo de captura individual de ventanas aplica ahora `MAP_SHOT_SCALE`
mediante un driver de cámara, antes de abrir la ventana y antes de refrescar
el render. Reutiliza el parser y los límites de zoom del mapa, detiene la
velocidad de cámara y solicita remapeo de culling/etiquetas cuando aplica,
sin rebajar una reconstrucción completa que ya estuviera pendiente. El log
`windows_shot: escala de cámara` registra la escala efectiva. El zoom del
mapa no cambia `SHOT_UI_SCALE` ni la cámara de preview. Sin una escala válida,
en capturas globales y fuera del modo de captura, conserva el comportamiento
anterior.

`window_shot_scale_updates_primary_camera_and_preserves_unspecified_state`
comprueba las cuatro escalas sobre una cámara real, los límites de zoom,
el remapeo solicitado, la cámara secundaria intacta y la ausencia de cambios
cuando no se solicita escala. No muta variables de entorno en los tests.
`MAP_SHOT_CENTER` sigue siendo exclusivo de la captura de mapa sin ventanas;
la captura individual de ventana conserva el centro de cámara de la partida.

## Validación ejecutada — 2026-09-08

La suite completa del cliente pasó con **1.190 tests aprobados y 2 opt-in
ignorados**, incluidos los sistemas de localización y el adaptador de zoom.
Clippy client/core con `--all-targets -- -D warnings`, formatter y el gate
directo de documentación también pasaron. La suite core del lote pasó sus
2.287 tests unitarios y sus integraciones.

Se ejecutó el binario real sobre Wayland, con un `XDG_CONFIG_HOME` temporal
para no modificar las preferencias del jugador. Xvfb no ofreció un surface
Vulkan compatible; esa corrida fallida no se cuenta como validación gráfica.

La ventana se capturó e inspeccionó **12 veces**: ES/EN por las seis escalas
ortográficas `0.25,0.5,1,2,4,8` (zoom `4×,2×,1×,0.50×,0.25×,0.125×`),
siempre a 1280×720 y UI scale 1. Cada log confirmó la escala efectiva. Título,
resumen, fila y controles usaron el idioma solicitado, conservaron el nombre
del vehículo `#0` y permanecieron visibles sin otra ventana vacía encima.
Las doce corridas salieron normalmente, sin panic ni conflicto ECS.

Fixture visual versionada: `mvp_openttd_rich.sav`, SHA-256
`0cc840af3f0d4c18e191a5c90ec83289d3026f537a3cfb66d414498d9e58bc9e`.
Binario comprobado: SHA-256
`2d65da196b9fb02ce793f0abffcde627211a965dd52fa5884d0a1e01eb61b294`.

La comprobación separada del mapa usó el `autosave0.sav` local (SHA-256
`dcaf2bdef2e744b91a0de319b4d35fb35bba404cb4a76e8dea674d60f7a6439f`),
centro `(128,128)`, resolución 1280×720 y escalas `1,2,4,8`. Se inspeccionaron
las cuatro capturas: terreno, árboles, edificios y minimapa permanecieron
visibles; no hubo pantalla de mapa negro ni panic. Es una comprobación de
ejecución y visibilidad en esa partida, no una comparación raster nativa.

Los PNG y logs locales de diagnóstico se conservan en
`/tmp/openttdrs-bounded-batch.992Zmh/`; no se incluyen en el repositorio ni se
consideran evidencia durable sin repetir las instrucciones anteriores.

## Alcance pendiente

Este corte no certifica igualdad raster de la ventana ni paridad general del
horario. Permanecen fuera su geometría y listado nativos, fechas de inicio,
arribos/salidas, edición avanzada, límites de velocidad, unidades y formato
numérico nativos, semántica de ejecución y los demás idiomas de OpenTTD.
#331 permanece abierto. Las capturas son diagnóstico complementario: el
contrato de esta etapa es el cambio de idioma sin alterar la partida.
El CI global tampoco se declara verde: #535 registra el fallo preexistente
del harness aislado de portabilidad documental, ajeno a estas tres
correcciones. Se deja pendiente para respetar el límite de tareas acordado.
