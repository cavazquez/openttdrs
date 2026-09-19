# Auditoría de dirección — 2026-09-18

Base: `ca698e5047d10e9497bd218fa53211be73e61ecb`, `main` publicado.
Referencia técnica: OpenTTD 15.3 `14ec60f248547d4d062a1160f0fc26d742319888`.
Decisión solicitada por el usuario: revisar el proyecto, cambiar dirección y
dejar abiertos sólo issues atómicos que se implementarán.

## Diagnóstico

El proyecto tiene mucha implementación y herramientas de contraste. El problema
de dirección es que el trabajo reciente optimiza contratos de paridad sin una
entrega de producto finita, y los issues amplían su alcance después de cada
subresultado. La documentación operativa mezcla estado, evidencia y prioridades.

Evidencia del corte:

- Los últimos 50 commits de la base auditada pertenecen a render, cámara,
  pruebas raster/paridad o documentación de paridad. Once son `docs(parity)`;
  veintiocho son `fix(render)`. Es un indicador de concentración del trabajo,
  no una medida de su calidad ni de todo el historial del proyecto.
- Hay **15 issues abiertos**, sin milestone asignado. El manifiesto local
  `active-backlog.json` todavía enumera sólo cinco padres y un corte del 5 de
  septiembre. La discrepancia no la detecta su checker local, que compara docs
  con JSON, no contra GitHub.
- El plan continuo tiene **10.001 líneas** y `PARIDAD.md` **7.158** antes de
  este cambio. El README aún prioriza RMAP-004, aunque los commits recientes
  trabajan sobre raster. El plan se separa de su historial en esta entrega.
- El [CI del HEAD auditado](https://github.com/cavazquez/openttdrs/actions/runs/35347362559)
  falla en rustfmt; el [replay de fuzz](https://github.com/cavazquez/openttdrs/actions/runs/35347362632)
  falla en metadata por `fuzz/Cargo.lock`, antes de ejecutar el corpus.
  [Plataformas](https://github.com/cavazquez/openttdrs/actions/runs/35347362549)
  y [documentación](https://github.com/cavazquez/openttdrs/actions/runs/35347362550)
  sí pasan. Por tanto, el commit no tiene validación CI completa.
- La consulta de releases no devuelve publicaciones y no hay PRs abiertos en
  esta revisión. `RELEASE_NOTES.md` contiene notas preparadas para una alpha;
  no demuestra que un paquete haya sido publicado.

## Revisión transversal

Se revisaron los tres crates, sus puntos de integración, pruebas representativas,
manifiestos, workflows, herramientas de release, matrices de compatibilidad,
los cuerpos de los 15 issues y sus comentarios recientes. Es una auditoría de
dirección y riesgos de integración; no una certificación línea por línea de todo
el código ni una nueva corrida de todas las matrices de paridad.

| Área | Evidencia inspeccionada | Decisión para la entrega |
|---|---|---|
| Arquitectura | `core` contiene estado/comandos/tick, cliente Bevy adapta UI/render y `net` transporta lockstep; `docs/ARCHITECTURE.md` ya define incrementos jugables | Conservar las capas; no reescribir el motor |
| Simulación y economía | `tests/sp1_playable_cycle.rs` carga, entrega y cobra, pero inserta vehículo y stocks directamente | Añadir una ruta completa por comandos públicos, con mina y central que correspondan al cargo |
| Movimiento | Existen tests/oráculos road, rail/PBS y Airport FTA; #330 pide una matriz extensa de geometrías | Certificar un servicio vial concreto; aplazar ampliaciones generales |
| Mundo procedural | `random-map-matrix.md` contiene una cohorte exacta de 15 mapas; RMAP conserva gaps fuera de ella | Reutilizar lo existente y congelar la expansión multiclima durante este corte |
| Persistencia | JSON versionado, SAV parcial y tests de hotkeys; `save_load_after_n_steps` sólo compara el tick de un mundo 5×5 | Verificar continuación de una ruta cargada; mantener límites SAV explícitos |
| Render | Herramientas raw/semántica/draw/raster y muchos arreglos focales; paridad global pendiente | Retener regresiones y corregir bloqueos visibles del recorrido; suspender exploración raster abierta |
| UI y bootstrap | Menú, herramientas, showcase, objetivos/historia y ES/EN ya existen; demo mezcla transportes; F5/F9 sólo reportan por log | Entrada Primera ruta, objetivo visible y feedback de persistencia |
| Red | `tcp_lockstep.rs` y `host_migration.rs` cubren integración; algunos tests permiten salir si sockets están prohibidos | Mantener soporte existente; no certificarlo por una ejecución restringida ni ampliar su alcance |
| NewGRF | Matrices Action0/callback distinguen parseado y runtime; quedan scopes, FTA y persistencia residuales | Fuera del escenario; mantener cobertura existente sin prometer soporte universal |
| Distribución | Release workflow, assets, checks SAV y smoke de handshake ya implementados | Extender el smoke Linux al menú/render del paquete real; aprovechar el workflow existente |

## Comprobaciones ejecutadas en esta auditoría

| Comprobación | Resultado |
|---|---|
| `git status --porcelain --untracked-files=no` inicial | Sin cambios versionados; se preservan todos los archivos locales no versionados |
| `cargo fmt --all -- --check` | Falla en tres bloques de `core/src/map/station_tile_anim.rs` |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Falla: 261 diagnósticos en lib test del core |
| `cargo clippy -p openttdrs-client --all-targets --locked --offline -- -D warnings` | Falla: 266 diagnósticos en tests del cliente |
| `cargo test -p openttdrs-core --test sp1_playable_cycle --locked --offline --quiet` | 1 aprobado, 0 fallidos, 0 ignorados; alcance sintético descrito arriba |

La auditoría no ejecutó una partida manual de 15 minutos, un release dry-run ni
las suites completas otra vez. Esas aceptaciones siguen pendientes. Los fallos
anteriores se convierten en #568–#571; no se ocultan bajo checks parciales.

## Decisión y alternativas

El siguiente corte certificará **primera ruta jugable en solitario**:
Temperate 64×64, carretera, carbón mina → central, compra y órdenes por comandos,
guardado JSON y continuación. Menú/guía ES/EN, OpenGFX y paquete Linux.

Seguir reduciendo el diff de Kale aporta fidelidad, pero no resuelve los gates
rotos ni certifica ese recorrido. Abrir todos los contratos SAV/NewGRF/mapa a
la vez perpetuaría el alcance indefinido. Una reescritura descartaría capas y
pruebas útiles sin evidencia de necesidad. Se elige consolidar la experiencia
existente y medirla con un criterio de salida finito.

Coste asumido: la paridad global y otras plataformas/transportes no reciben
ampliación funcional en este corte. Se conservan el código, tests, artefactos
y límites documentados. La aceptación de producto no se llama paridad OpenTTD.

## Disposición de todos los issues anteriores

Se retiran los 15 como **not planned** para este corte. Es un cierre de
planificación solicitado, no un cierre por implementación. Los enlaces
preservan cuerpos, comentarios y evidencia; ninguna capacidad pendiente se
convierte en «hecha». Los sucesores son #568–#577 y tienen contratos nuevos,
no heredan el alcance completo de los padres.

| Issue anterior | Motivo del retiro y brecha conservada |
|---|---|
| [#326](https://github.com/cavazquez/openttdrs/issues/326) | Compositor/framebuffer global sin límite de cohorte; residual en `PARIDAD.md` y contratos raster |
| [#328](https://github.com/cavazquez/openttdrs/issues/328) | Interoperabilidad SAV amplia; conservar matriz SAV. #573 cubre exclusivamente continuación JSON |
| [#329](https://github.com/cavazquez/openttdrs/issues/329) | Runtime NewGRF residual de varias familias; conservar matrices Action0/callback |
| [#330](https://github.com/cavazquez/openttdrs/issues/330) | Matriz road/rail abierta; #572 certifica sólo una ruta por comandos, sin absorber su paridad general |
| [#331](https://github.com/cavazquez/openttdrs/issues/331) | Catálogos/locale generales; el corte usa ES/EN existentes con textos nuevos acotados |
| [#338](https://github.com/cavazquez/openttdrs/issues/338) | Mezcla multiclima, settings, generación y runtime industrial; conservar RMAP como inventario de brechas |
| [#499](https://github.com/cavazquez/openttdrs/issues/499) | Comentarios acreditan implementación vanilla y scheduler; el residual se expandió a NewGRF/settings/matriz temporal. No certificamos el resto al retirarlo |
| [#512](https://github.com/cavazquez/openttdrs/issues/512) | La frontera day[1] ya fue superada según su evidencia; permaneció abierto por runtime general. Conservar trazas sin prolongar el contrato |
| [#527](https://github.com/cavazquez/openttdrs/issues/527) | El título vanilla terminó incluyendo callbacks/sonidos de casas NewGRF; queda evidencia runtime extendida por certificar |
| [#561](https://github.com/cavazquez/openttdrs/issues/561) | Orden y composición de vidrio ferroviario fuera del servicio vial; conservar evidencia, sin afirmar aceptación global |
| [#563](https://github.com/cavazquez/openttdrs/issues/563) | TileLayout NewGRF de road stops/waypoints fuera del escenario vanilla |
| [#564](https://github.com/cavazquez/openttdrs/issues/564) | Foundations/rotaciones AirportTile fuera del escenario |
| [#565](https://github.com/cavazquez/openttdrs/issues/565) | Tramtypes/catenaria custom fuera del escenario |
| [#566](https://github.com/cavazquez/openttdrs/issues/566) | Mezcla clipping, anchors, orden y framebuffer; se aplaza la certificación global |
| [#567](https://github.com/cavazquez/openttdrs/issues/567) | Depósito naval raster fuera del escenario; conservar evidencia parcial y sus límites |

## Seguimiento

El [plan corto](../parity/continuous-work-plan.md) es la fuente de orden y
aceptación. Abrir sólo trabajo necesario para esa entrega, con reproducción,
un resultado observable y un criterio de cierre; WIP de implementación = 1.
Al completarlo se decide el siguiente corte, sin reactivar automáticamente las
épicas retiradas. Esta auditoría termina con issues y docs sincronizados;
la implementación del nuevo backlog comienza por #568.

Verificación final de la reorganización: GitHub devuelve exactamente los diez
issues #568–#577 abiertos, todos en el hito `0.1 — vertical slice`; los quince
anteriores devuelven `CLOSED / NOT_PLANNED`, con explicación individual. El
manifiesto y el bloque activo de `PARIDAD.md` coinciden. Pasan
`check_parity_docs_fresh.sh`, los diez tests de `test_parity_docs_portability.py`
y `git diff --check`. El cuerpo del historial archivado se comparó contra el
archivo original en Git: se conserva íntegro, aparte del título y aviso de
archivo. No se modificó código de juego, CI ni fixtures en esta etapa.
