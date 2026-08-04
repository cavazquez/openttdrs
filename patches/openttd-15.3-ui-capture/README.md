# OpenTTD 15.3 UI capture driver (openttdrs #297)

Este parche se aplica al commit fijado en
[`docs/parity/openttd-reference.json`](../../docs/parity/openttd-reference.json).
Abre de forma determinista una ventana de la primera familia visual y pide una
captura al proveedor PNG nativo de OpenTTD. No altera el comportamiento cuando
las variables de entorno no están presentes.

Durante la captura congela el tick de juego y deja que el bucle de interfaz
avance unos frames; así vehículos, fecha y animaciones no dependen del tiempo
de arranque de la máquina.

```bash
./patches/openttd-15.3-ui-capture/integrate.sh
cmake -S reference/openttd-upstream -B /tmp/openttdrs-openttd-15.3-ui \
  -DOPTION_USE_ASSERTS=OFF
cmake --build /tmp/openttdrs-openttd-15.3-ui --target openttd
```

El driver acepta `Vehicle`, `Orders`, `Timetable`, `Depot`, `Town` e
`Industry`. La captura se realiza sobre el save entregado con `-g`; si la
entidad requerida no existe, OpenTTD sale y no deja PNG, lo que el gate trata
como fallo. `OPENTTDRS_UI_CAPTURE_NAME` sólo admite un basename ASCII seguro;
OpenTTD lo escribe en su directorio aislado de `screenshot/`.

El script [`scripts/capture_window_visual_baselines.sh`](../../scripts/capture_window_visual_baselines.sh)
prepara el perfil aislado, pide las cuatro resoluciones/escalas y mueve el PNG
resultante a la ruta versionada esperada. Las plantillas `configs/gui-scale-*.cfg`
son parte del parche: evitan que la escala dependa de la configuración del usuario.
