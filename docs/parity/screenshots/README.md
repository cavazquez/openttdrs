# Screenshots de paridad

Capturas de evidencia visual. Documentación viva: [../../PARIDAD.md](../../PARIDAD.md).

## Ventanas y subventanas

La matriz tipada `WINDOW_PARITY_MATRIX`, en
`crates/openttdrs-client/src/ui/windows_shot.rs`, relaciona cada
`FloatingWindowId` con su familia, archivo/clase de OpenTTD 15.3, parent lógico
y condición de ventana upstream o extensión propia. Un test exige cobertura
exacta: una entrada por cada ID, sin faltantes ni duplicados.

`WINDOW_REFERENCE_GEOMETRY` registra los tamaños iniciales, mínimos, paso de
resize y política `auto`/`center` de `WindowDesc`. `spawn_floating_window`
aplica la variante primaria al crear el marco (#243): width/height conocidos,
centrado cuando corresponde, y clamp que evita toolbar/statusbar a 1280×720.
Las preferencias pueden persistir `Id=x,y` o `Id=x,y,w,h`.

La matriz completa puede exportarse como JSON para tooling/CI:

```bash
OPENTTDRS_WINDOW_MATRIX=/tmp/openttdrs-window-matrix.json \
  cargo run -p openttdrs-client
```

El archivo se escribe al instalar el plugin de UI; se puede cerrar el cliente
una vez iniciado. El JSON (schema v2) incluye commit upstream, `rust_impl`,
`capture_stem`, `window_key` (class + instance), `known_gaps` (issue por
categoría) y un bloque `report.missing_captures` para ausencias no silenciosas.

La referencia upstream es el commit `14ec60f248547d4d062a1160f0fc26d742319888`
(tag 15.3), registrado en `../openttd-reference.json`.

La build gráfica oficial se prepara sin modificar el checkout y valida commit,
versión y OpenGFX 8.0:

```bash
bash scripts/build_openttd_ui_reference.sh
```

Dependencias mínimas en Debian/Ubuntu: `libsdl2-dev`, `libpng-dev`,
`libfreetype6-dev` y `liblzma-dev`. El script rechaza una build sin LZMA porque
no puede cargar `opntitle.dat`, aunque el enlace del ejecutable haya terminado
sin error.

El binario queda por defecto en `/tmp/openttdrs-openttd-15.3/openttd`. Para que
las referencias no dependan de configuración, autosaves o contenido personal,
debe ejecutarse con un perfil XDG aislado:

```bash
XDG_DATA_HOME=/tmp/openttdrs-ref-data \
XDG_CONFIG_HOME=/tmp/openttdrs-ref-config \
  /tmp/openttdrs-openttd-15.3/openttd \
  -v sdl -s null -m null -r 1280x720 -I OpenGFX -x -Q
```

Para capturar la composición completa en las dos resoluciones base:

```bash
bash scripts/capture_ui_reference.sh
```

Para aislar una ventana por su `storage_key`:

```bash
OPENTTDRS_WINDOW_SHOT_ID=Vehicle bash scripts/capture_ui_reference.sh
OPENTTDRS_WINDOW_SHOT_ID=Orders bash scripts/capture_ui_reference.sh
```

`TownAuthority` ofrece además estados reproducibles para sus acciones:

```bash
OPENTTDRS_WINDOW_SHOT_ID=TownAuthority \
OPENTTDRS_TOWN_AUTHORITY_SHOT_STATE=no-funds \
  bash scripts/capture_ui_reference.sh
OPENTTDRS_WINDOW_SHOT_ID=TownAuthority \
OPENTTDRS_TOWN_AUTHORITY_SHOT_STATE=unavailable \
  bash scripts/capture_ui_reference.sh
```

Para comprobar escalado:

```bash
OPENTTDRS_WINDOW_SHOT_ID=Vehicle OPENTTDRS_SHOT_UI_SCALE=2 \
  bash scripts/capture_ui_reference.sh
```

Las salidas quedan como `1280x720/window_<id>_<scale>x.png` y
`1920x1080/window_<id>_<scale>x.png`. Un ID desconocido no genera captura y se
informa como error; antes de incorporar una referencia se debe usar una clave
presente en la matriz. Las escalas aceptadas van de 0.5× a 4×.

## Gate visual por familia (#297, #299, #300)

La fase 1 cubre `Vehicle`, `Orders`, `Timetable`, `Depot`, `Town` e `Industry`.
La fase 2 añade los pickers `RailStation`, `Airport`, `RoadStop`, `Object`,
`Bridge`, `Dock`, `Buoy`, `RailWaypoint`, `RoadWaypoint`, `Tree`, `Terraform`,
`Sign`, `DepotBuild` y `Signal`.
La fase 3 añade `Finances`, `CompanyView`, `GraphIncome`,
`GraphOperatingProfit`, `GraphCompanyValue`, `CargoPaymentRates`,
`SubsidyList`, `League`, `NewsHistory` y `NewsSettings`. En OpenTTD 15.3 la
última no es una ventana separada: su referencia explícita es `Game Options`,
registrada como `settings/game-options` en `capture_route`. El manifiesto
conjunto es
[`window-regression.json`](window-regression.json); cada perfil vive en:

```text
window-regression/<id>/<resolución>-<escala>x/
  reference.png   # OpenTTD 15.3, commit fijado
  candidate.png   # openttdrs
  diff.png        # RGBA, generado determinísticamente
  sidecar.json    # hashes, geometría, métricas y tolerancia
```

El driver C++ versionado en
[`patches/openttd-15.3-ui-capture/`](../../../patches/openttd-15.3-ui-capture/)
abre la ventana del fixture sin usar estado de usuario ni configuraciones
persistentes. La fase 2 activa la ruta real de cada picker (toolbar, selector o
chooser); `capture_route` queda en el manifiesto y el sidecar para hacer esa
apertura auditable. Vehicle, Orders, Timetable y Town usan
`mvp_openttd_rich.sav`; Depot e Industry usan `rail_signals_mixed.sav`; los
pickers construction usan `mvp_openttd_rich.sav`. El candidato carga exactamente
el mismo `.sav` mediante `OPENTTDRS_SAV_LOAD`, y ambos drivers congelan el tick
de simulación antes de medir. Para generar/actualizar los cuatro artefactos de
cada perfil se requieren `xvfb`, `xauth` y `weston`: OpenTTD se
ejecuta en Xvfb y el cliente en Weston headless con renderer GL. Así ambos
procesos reciben una superficie virtual de la resolución pedida y se rechaza
un compositor que silenciosamente devuelva 1280×720 al solicitar 1920×1080.

```bash
sudo apt install xvfb xauth weston
./patches/openttd-15.3-ui-capture/integrate.sh
cmake -S reference/openttd-upstream -B /tmp/openttdrs-openttd-15.3-ui \
  -DOPTION_USE_ASSERTS=OFF
cmake --build /tmp/openttdrs-openttd-15.3-ui --target openttd
OPENTTDRS_UI_CAPTURE_BIN=/tmp/openttdrs-openttd-15.3-ui/openttd \
  bash scripts/capture_window_visual_baselines.sh
```

La comprobación no requiere Pillow ni ImageMagick y falla ante artefacto
ausente, dimensión distinta, sidecar desactualizado o diff por encima de la
tolerancia declarada:

```bash
python3 scripts/window_visual_regression.py
```

Durante la regeneración local puede limitarse a una ventana sin que las otras
ausencias oculten el resultado:

```bash
OPENTTDRS_WINDOW_CAPTURE_IDS=Vehicle \
  OPENTTDRS_UI_CAPTURE_BIN=/tmp/openttdrs-openttd-15.3-ui/openttd \
  bash scripts/capture_window_visual_baselines.sh
python3 scripts/window_visual_regression.py --window Vehicle
```

El reporte JSON etiqueta separadamente `absence`, `geometry`, `iconographic`
y `chromatic`. Las diferencias aceptadas sólo pueden declararse con su categoría
y un issue abierto; no hay tolerancias implícitas. La prueba de mutación y el
gate del manifiesto canónico forman parte de los checks Python compartidos de
CI: una captura ausente, una dimensión distinta, un hash/sidecar obsoleto o
una diferencia fuera de tolerancia no pueden pasar silenciosamente.

Las demás ventanas todavía son el inventario de #240. Las seis de la fase 1
citan #297, los 14 pickers construction de la fase 2 citan #299 y las diez
entradas economy/reports de la fase 3 citan #300; así una ausencia no queda sin
dueña. La ampliación continúa con settings/dialogs y, por último, el resto de
`WINDOW_PARITY_MATRIX` hasta sus 62 entradas.
