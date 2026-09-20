# V1-UI — edición de Órdenes

Contrato acotado de [#590](https://github.com/cavazquez/openttdrs/issues/590).
Certifica la ventana **Órdenes** del camión demo `MPS Mail Truck #9010`, no la
paridad completa de todas las órdenes o del aspecto de OpenTTD.

## Recorrido funcional

`v1_orders_ui_edits_demo_truck_route_and_runs_it` usa la ruta de producción:

- selecciona el camión desde el mapa y abre `Órdenes` mediante el botón de su
  ventana;
- vacía las dos órdenes iniciales usando dos veces el botón visible
  `Eliminar`, sin mutar la lista desde la prueba;
- inicia `Ir a`, añade la estación de carga y la de entrega por clics de mapa,
  y exige la lista exacta `[Station(carga), Station(entrega)]`;
- borra y vuelve a añadir el segundo destino por la misma UI;
- ejecuta 1.200 ticks y verifica que el camión transportó carga, visitó la
  entrega y conserva exactamente esas dos órdenes.

Se ejecuta así desde la raíz:

```bash
cargo test --locked --offline -p openttdrs-client v1_orders_ui_edits_demo_truck_route_and_runs_it -- --nocapture
```

El recorrido conserva los flujos normales de menú y el feedback F5/F9; no
restaura ni presenta el atajo histórico «First Route».

## Certificación visual

La evidencia versionada está en
[`screenshots/orders-v1-regression.json`](screenshots/orders-v1-regression.json).
Ejecuta la demo integrada, sin cargar saves personales, para ES/EN ×
1280×720/1920×1080 × escala 1/2: ocho capturas runtime en total.

Los ROIs son rectángulos fijos del panel completo y no aplican máscaras
dinámicas. Para hacer reproducible el oracle, el modo de captura individual de
Órdenes ubica el panel en una posición determinista; la colocación normal de
ventanas en el juego no cambia. El presupuesto V1 es exactamente:

- delta RGBA máximo por píxel: `8`;
- píxeles sobre ese delta: hasta `0,5 %` dentro de cada ROI;
- media absoluta normalizada por canal: hasta `1/255`.

En el SHA `72d906d8ed53532ba33c7c6fbe4eaf39fbf946ae`, las ocho comparaciones
dan `0` píxeles sobre el umbral y media `0`. Los sidecars enlazan cada PNG a
ese SHA, a su comando de captura y a las 19 aserciones `present` +
`actionable`: lista, horario, carga/descarga, non-stop, andén, depósito,
refit, reordenamiento, salto, borrado, destino, órdenes compartidas y las tres
acciones condicionales.

Para regenerar y validar la evidencia localmente:

```bash
bash scripts/capture_orders_v1_certification.sh baseline
bash scripts/capture_orders_v1_certification.sh candidate
bash scripts/check_orders_v1_certification.sh
```

La captura requiere Weston headless; la verificación versionada no requiere un
compositor. `./scripts/check.sh parity-docs` incluye el último comando. La
certificación sigue ligada a que CI quede verde para el SHA publicado; no
convierte otras ventanas ni la UI completa en paridad aceptada.
