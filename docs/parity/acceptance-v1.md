# Contrato V1 — verde acotado por área

Fecha local: **2026-09-20**. Base inspeccionada: `84144747`; `origin/main`
observado al auditar: `340585dc147aab2fad64d3427fae5cfb785aa3c1`.
Referencia técnica: OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`.

La solicitud de actualizar el resumen, acotar tolerancias y crear issues
atómicos abre este corte. Reemplaza la instrucción anterior de trabajar sólo
en Snap; conserva los quince padres históricos como `not planned`.
Esta entrega define aceptación y backlog, no implementa todos los pendientes.

## Qué significa verde

Cada verde se limita al escenario, entradas, campos y horizonte publicados.
Una fila pasa cuando todos sus contratos obligatorios tienen evidencia del
mismo candidato; no basta contar issues cerrados. Mantener separados:

- capacidad implementada;
- aceptación V1 reproducida;
- paridad general, que sigue parcial en las matrices canónicas.

Los escenarios de servicio usan Temperate 64×64, seed/settings explícitos,
OpenGFX, una compañía salvo prueba de ownership/red/IA, sin averías ni
desastres. Sólo el caso NewGRF instala su GRF mínimo. El mapa puede fijar
pueblos/industrias; la operación crea infraestructura, vehículos y órdenes
mediante comandos productivos y no inyecta pagos o carga.
Kale y las fixtures externas de PBS/FTA conservan su configuración original.

Fuera de V1: todos los climas/seeds/tamaños, compatibilidad SAV universal,
NewGRF arbitrarios, tráfico ilimitado, raster global en todos los zooms,
las 57 ventanas, NoAI/GameScript Squirrel y protocolo de red OpenTTD.
Esos límites deben seguir visibles aun cuando V1 esté completamente verde.

## Tolerancias vinculantes del corte

Son presupuestos de aceptación fijados ahora, no resultados ya obtenidos ni
estimaciones de error inevitable. Los issues de gates los harán ejecutables;
no se han relajado ni reescrito las tolerancias/goldens existentes.

### Estado y comportamiento

- **Cero diferencias** en campos enteros, órdenes, carga, dinero, ownership y
  RNG que declare cada contrato. El hash canónico complementa las aserciones
  de campos; no sustituye comprobar los datos que el hash no cubra.
- Comparación contra OpenTTD sólo en los campos/fixtures con oracle externo.
  Repetir dos veces Rust demuestra determinismo propio, no paridad nativa.
- Una ruta de servicio entrega carga real y cobra antes de **40.000 ticks**.
  No se exige beneficio neto durante la primera entrega.
- Guardar/cargar y la sesión de red se observan durante **2.000 ticks**.
  En red: dos clientes, segundo ingreso en tick 500, timeout **120 s**.
- Ausencia de herramienta, captura o conexión, `skip`, `ignored`, timeout o
  SHA distinto son **no ejecutado/fallo**, nunca éxito del contrato.

### Raster contra OpenTTD

Contrato V1-RAS: `Kale_TitleGame.sav`, SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`,
centro `(132,2)`, **800×600**, zoom nativo **Normal**, OpenGFX 8bpp,
`clean-static`, sin UI/vehículos/animación según el perfil existente.

Tres capturas por motor deben repetir su hash. Comparar el frame completo
con `dx=dy=0`, sin máscaras, realineación ni recortes adaptativos:

- RGBA distinto: **≤480 / 480.000 píxeles (0,1 %)**.
- Delta máximo de canal >64: **≤24 / 480.000 (0,005 %)**.
- Media absoluta RGBA por canal: **≤0,05**, sobre escala **0..255**.
- Cobertura faltante: **0 píxeles**. Se deben cumplir los cuatro límites.

La adenda del 18 de septiembre registró 45 diferencias en Normal; es evidencia
histórica que hizo razonable intentar este contrato. Out2x (4.427), Out4x
(87.289) y Out8x (71.239) seguían divergentes en esa misma escena. Los otros
cinco zooms se registran como diagnóstico, sin contarlos como verde raster. Si
se cambia cámara/culling/overview, se ejecutan las seis escalas según
`AGENTS.md` y se investigan las regresiones.

[#589](https://github.com/cavazquez/openttdrs/issues/589) instrumenta
`capture_v1_raster.sh` y `v1_raster_gate.py`: exige tres hashes iguales por
motor, `dx=dy=0`, los cuatro límites combinados, assets/pins/logs y los cinco
diagnósticos. Sus pruebas cubren cada frontera del presupuesto, evidencia
faltante, geometría inválida y no determinismo. La corrida real de
`1f170b076ce4f79c961082277cf5a35877e5d930` pasó con 437 píxeles, 5 >64,
media 0,0210869792 y cobertura 0; ver [evidencia Kale V1](raster-v1-kale.md).
El cierre de la issue aún exige CI remota verde del SHA vigente.

### UI propia

Contrato V1-UI: ventana Órdenes, dos destinos de un camión, ES/EN ×
1280×720/1920×1080 × escala UI 1/2 = **8 perfiles**. ROIs de panel, lista y
botones fijos antes de medir; cero controles/textos requeridos ausentes,
acciones inaccesibles o errores de estado.

La regresión compara una captura **nueva del candidato** contra un baseline
aprobado del propio cliente: en cada ROI, píxeles con delta máximo RGBA >8
**≤0,5 %** y media por canal **≤1** sobre escala 0..255. No se promedian
regiones para esconder un botón roto. No se redefine ese baseline sólo porque
falló el test; un cambio visual intencional requiere evidencia y revisión.

La comparación histórica con OpenTTD sigue siendo diagnóstica: las 57 entradas
de `screenshots/window-regression.json` permiten actualmente ratio **1** y
deltas **255**. Orders/1280x720-1x registra **921.572/921.600** píxeles
distintos. Verificar sus hashes/sidecars prueba integridad del archivo, no
similitud ni ausencia de regresiones en el ejecutable actual. [#584](https://github.com/cavazquez/openttdrs/issues/584)
ya separa roles, exige SHA/procedencia fresca y aplica el presupuesto fijo a
manifiestos `client_regression`; [#590](https://github.com/cavazquez/openttdrs/issues/590)
aporta las ocho capturas runtime de Órdenes en
[`orders-v1-regression.json`](screenshots/orders-v1-regression.json), ligadas
al SHA `72d906d8ed53532ba33c7c6fbe4eaf39fbf946ae`. La categoría
`client_regression` pasa con sus sidecars y presupuesto fijo; no se exige
clonar la apariencia de OpenTTD.

### Paquetes

Menú desde el archivo distribuible extraído, cwd/perfil temporal y assets del
paquete; frame no vacío **1280×720**. Para Windows/macOS, ES/EN y timeout de
**60 s por idioma**. Cero assets ausentes, panic o escritura dentro de `$SNAP`.
Conservar SHA del paquete, SHA fuente, logs y captura del entorno real.
Falta de sesión gráfica es un bloqueo visible, nunca un smoke aprobado.

## Contratos pendientes e issues

Cada issue tiene evidencia, un entregable revisable, criterios, dependencias
y exclusiones. Su aceptación no crece después de la primera mejora. Una
causa independiente requiere otra tarea; los criterios no satisfechos siguen
pendientes. Las dependencias por ID V1 resuelven a los enlaces de esta lista.

- **V1-VIS — [#584](https://github.com/cavazquez/openttdrs/issues/584):** roles diagnóstico/certificación, presupuesto fijo y procedencia de captura (implementado; #590 aporta el caso Órdenes).
- **V1-NETG — [#585](https://github.com/cavazquez/openttdrs/issues/585):** red obligatoria: `PermissionDenied` falla con diagnóstico; inyección negativa y 22 recorridos loopback reales (23 pruebas, 0 ignoradas).
- **V1-PAY — [#586](https://github.com/cavazquez/openttdrs/issues/586):** 11 cargos × 3 cantidades × 2 distancias × 3 edades = 198 pagos nativos exactos ([oráculo](temperate-payment-oracle.md) implementado; fixture/procedencia con hash).
- **V1-PKT — [#587](https://github.com/cavazquez/openttdrs/issues/587):** una transferencia de carbón, dos tramos, conservación y feeder share ([contrato/oráculo](coal-transfer-v1.md) implementado; falta CI remota vigente antes del cierre).
- **V1-SAV — [#588](https://github.com/cavazquez/openttdrs/issues/588):** una orden `Station` mutada por `SetSharedOrderAt`, preservada después del re-guardado nativo; mantiene las 6 cargas, el roundtrip obligatorio y hashes/IDs/ORDL en [su evidencia](sav-ordl-v1.md). Implementado; espera CI remota vigente antes del cierre.
- **V1-RAS — [#589](https://github.com/cavazquez/openttdrs/issues/589):** gate y evidencia de la escena Normal con el presupuesto anterior.
- **V1-UI — [#590](https://github.com/cavazquez/openttdrs/issues/590):** edición de órdenes, 8 perfiles; depende de V1-VIS.
- **V1-OWN — [#591](https://github.com/cavazquez/openttdrs/issues/591):** compra, start/stop, órdenes y demolición ajenas: [4 rechazos atómicos, 4 controles propios e issuer inválido](ownership-v1.md) con el API productivo de red; implementado, a la espera de CI remota vigente antes del cierre.
- **V1-GRF — [#592](https://github.com/cavazquez/openttdrs/issues/592):** un camión CB36 reatachado al catálogo tras JSON, 2.000 ticks, con bytes GRF V2 propios, GRFID/versión/parámetros/SHA y `7C` documentados en [su evidencia](newgrf-cb36-v1.md).
- **V1-SHIP — [#593](https://github.com/cavazquez/openttdrs/issues/593):** primera entrega marítima con boya, sin locks/canales.
- **V1-AIR — [#594](https://github.com/cavazquez/openttdrs/issues/594):** un avión vanilla entre dos aeropuertos Country, primera entrega pagada.
- **V1-NET — [#595](https://github.com/cavazquez/openttdrs/issues/595):** dedicated, dos clientes y late join, 2.000 ticks; depende de V1-NETG.
- **V1-AIT — [#596](https://github.com/cavazquez/openttdrs/issues/596):** primera entrega ferroviaria pagada de TransCargo; [fixture determinista y aislamiento multicompañía](transcargo-v1.md) implementados, a la espera de CI remota vigente antes del cierre.
- **V1-AIRH — [#597](https://github.com/cavazquez/openttdrs/issues/597):** primera entrega de pasajeros pagada de RoadHaul; [fixture determinista, log de eventos y aislamiento multicompañía](roadhaul-v1.md) implementados, a la espera de CI remota vigente antes del cierre.
- **V1-GSCOUNT — [#598](https://github.com/cavazquez/openttdrs/issues/598):** `CargoDelivered` ya consulta el ledger persistente de unidades finales del tipo solicitado, sin inferir tipos del agregado legacy; [regresiones y migración](gs-cargo-filter-v1.md) implementadas, a la espera de CI remota vigente antes del cierre.
- **V1-GS — [#599](https://github.com/cavazquez/openttdrs/issues/599):** progreso y noticia única después de JSON; depende de V1-GSCOUNT.
- **V1-EDIT — [#600](https://github.com/cavazquez/openttdrs/issues/600):** persistir un escenario editado y abrirlo para jugar.
- **V1-WIN — [#601](https://github.com/cavazquez/openttdrs/issues/601):** menú del ZIP Windows en entorno gráfico real.
- **V1-MAC — [#602](https://github.com/cavazquez/openttdrs/issues/602):** menú del tar.gz macOS arm64 en entorno gráfico real.
- **V1-REPORT — [#603](https://github.com/cavazquez/openttdrs/issues/603):** resultado por contrato/SHA; rechazar informes ausentes o mezclados. Su implementación puede comenzar con casos sintéticos; declarar todo verde depende de todos los contratos de la fila.

Se conservan **[#582](https://github.com/cavazquez/openttdrs/issues/582)**
y **[#583](https://github.com/cavazquez/openttdrs/issues/583)** para Snap:
el commit `84144747` incluye el fix y smoke read-only local, pero eso no
acredita una nueva revisión instalada desde la tienda. No se duplican esos
issues ni se cierran como parte de este trabajo documental.

## Evidencia comprobada al preparar el corte

Sobre `84144747`, sin cambios de runtime en esta entrega:

```bash
cargo test --locked --offline -p openttdrs-core --lib command::tests::rail::construction
cargo test --locked --offline -p openttdrs-core --lib command::tests::road::
cargo test --locked --offline -p openttdrs-core \
  --test pbs_openttd_oracle --test pbs_dual_curve_oracle \
  --test consist_pbs_openttd_oracle --test sp1_playable_cycle \
  --test airport_fta_openttd_oracle
```

Resultado: **21 + 54 + 5 + 3 + 6 + 2 + 4 = 95 tests**, cero fallos y cero
ignorados en esta selección. PBS conserva comparaciones exactas de las
fixtures; los 4 tests de Helidepot no certifican la ruta de ala fija V1-AIR.
La construcción, las fixtures PBS y la ruta vial/JSON tienen verde acotado;
los demás contratos siguen pendientes.

No se ejecutó una CI completa para esta actualización ni se regeneró la
matriz raster: el verde remoto debe comprobarse en el SHA que se certifique.
Los reportes de 2026-09-07/18 siguen identificados como evidencia histórica.

## Orden y cierre

Orden operativo en [continuous-work-plan.md](continuous-work-plan.md).
El [manifiesto](active-backlog.json) y el bloque activo de
[PARIDAD.md](../PARIDAD.md#backlog-sucesor-activo) deben coincidir con GitHub.
Su checker local no consulta el servicio remoto.

No publicar porcentajes globales a partir de una sola fixture. No borrar
tests, esconder skips, aumentar presupuestos ni sustituir referencias para
convertir un fallo en verde. Si V1 necesita un alcance diferente, registrar
explícitamente otro contrato; no reinterpretar el actual después de medirlo.
