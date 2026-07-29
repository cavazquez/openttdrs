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

Para comprobar escalado:

```bash
OPENTTDRS_WINDOW_SHOT_ID=Vehicle OPENTTDRS_SHOT_UI_SCALE=2 \
  bash scripts/capture_ui_reference.sh
```

Las salidas quedan como `1280x720/window_<id>_<scale>x.png` y
`1920x1080/window_<id>_<scale>x.png`. Un ID desconocido no genera captura y se
informa como error; antes de incorporar una referencia se debe usar una clave
presente en la matriz. Las escalas aceptadas van de 0.5× a 4×.

Las capturas por ventana son el insumo visual de #240. Mientras falte
`1280x720/window_<id>_1x.png`, el reporte JSON y los tests de
`windows_shot` citan el issue #240 (no toleran la ausencia en silencio).
Las diferencias deben clasificarse en chrome/iconos (#241), lifecycle (#242),
geometría (#243) o la familia funcional correspondiente (#244–#248), sin
aceptar tolerancias anónimas.
