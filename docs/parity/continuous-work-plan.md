# Corte V1 — aceptación acotada por área

Actualizado: **2026-09-20**. Base inspeccionada: `84144747`.

La nueva solicitud del usuario abre un corte finito para el resumen del README:
actualizar qué está hecho, fijar qué significa verde y crear tareas atómicas.
El [contrato V1](acceptance-v1.md) define escenarios, tolerancias y exclusiones.
Se crean #584–#603 y se conservan #582/#583; no se reabren épicas históricas.
La entrega documental prepara el trabajo posterior, no autoriza a afirmar que
todos los contratos ya pasan. La paridad global continúa como referencia
técnica separada. El escenario guiado anterior permanece retirado; sus fixtures
siguen siendo evidencia interna.

La entrega está publicada como
[prerelease `v0.1.0-alpha.1`](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
y [Snap Store `latest/edge`](https://snapcraft.io/openttdrs) (revisión 1,
Linux amd64). La revisión 1 tiene un defecto posterior de empaquetado: intenta
materializar tiles OpenGFX dentro del mount `$SNAP` de sólo lectura. [#582](https://github.com/cavazquez/openttdrs/issues/582)
y [#583](https://github.com/cavazquez/openttdrs/issues/583) conservan ese alcance.
`84144747` ya aporta build y smoke read-only locales de `0.1.0-alpha.2`;
el cierre exige la evidencia restante del paquete instalado/distribuido.

## Entrega alpha anterior (cumplida)

La validación construyó un servicio de camiones entre una mina de carbón y una
central eléctrica, recibió el primer pago, guardó JSON, cargó y completó otra
entrega. El paquete Linux debía funcionar fuera del checkout. El fixture usa un
escenario Temperate plano 64×64, año 1950, una compañía y OpenGFX, sin NewGRF,
IA, desastres ni averías.

Se reutilizaron comandos, simulación, persistencia y empaquetado existentes.
La entrada exclusiva de menú y su guía se retiraron tras la aceptación: Nueva
partida, carga, escenarios, editor y demo son las vías actuales. La evidencia
interna no requiere paridad raster perfecta ni equivalencia universal de saves
o generación.

## Evidencia publicada del corte

- Distribución alpha — el tag
  [`v0.1.0-alpha.1`](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
  produjo los paquetes de GitHub para Linux x86_64, Windows x86_64 y macOS
  arm64. El Snap estricto `core24` se compiló con la misma versión, pasó los
  linters de Snapcraft con sólo avisos de bibliotecas X11 preventivas y se
  publicó como revisión 1 de `latest/edge`. El launcher y los assets incluidos
  pasaron `--check-assets` desde un árbol empaquetado escribible, pero no desde
  el mount Snap real. La corrección posterior #582/#583 cubre esa diferencia
  con materialización en build y un smoke read-only.
- [#568](https://github.com/cavazquez/openttdrs/issues/568) — `rustfmt` de
  `station_tile_anim.rs`, cerrado con el commit `87114d5a` tras `cargo fmt
  --all -- --check` y revisión del diff.
- [#569](https://github.com/cavazquez/openttdrs/issues/569) — Clippy estricto
  de los tests del core, cerrado con `6cf0513b`; las excepciones quedaron
  acotadas a módulos de prueba y el crate pasa Clippy y sus tests.
- [#570](https://github.com/cavazquez/openttdrs/issues/570) — Clippy estricto
  de los tests del cliente, cerrado con `6567b735`; también pasa el chequeo
  estricto de todos los targets del workspace.
- [#571](https://github.com/cavazquez/openttdrs/issues/571) — lockfile y replay
  de fuzz, cerrado con `5565e802`; el
  [job remoto](https://github.com/cavazquez/openttdrs/actions/runs/35410088491)
  pasó con nightly fijado, corpus, sanitizers y ancla SAV.
- [#572](https://github.com/cavazquez/openttdrs/issues/572) — fixture vial y
  ruta de carbón construida sólo mediante comandos, cerrado con `53d727c9`; la
  entrega, el pago y el hash canónico se verifican dos veces y el estado
  posterior conserva ese hash al serializar y recuperar JSON.
- [#573](https://github.com/cavazquez/openttdrs/issues/573) — continuación
  productiva de ese fixture tras guardar JSON, cerrada con `d55686ae`; con carga
  a bordo, ambas ramas conservan hash, vehículo, órdenes, carga, stocks,
  ingresos y RNG durante 2.000 ticks, y la recargada vuelve a entregar y cobrar.
- [#574](https://github.com/cavazquez/openttdrs/issues/574) — la entrada de
  menú temporal usada en la aceptación se cerró con `64ee70db`. Fue retirada
  del cliente al cerrar este corte; el fixture no se expone como una sesión
  separada.
- [#575](https://github.com/cavazquez/openttdrs/issues/575) — el panel de
  objetivo y avance usado durante la aceptación se cerró con `ff1b4730`. Se
  retiró junto con esa entrada; la cobertura de simulación permanece en tests
  internos de comandos y persistencia.
- [#576](https://github.com/cavazquez/openttdrs/issues/576) — resultado visible
  de F5/F9, cerrado con `77589c67`; guardar, cargar y sus errores generan un
  aviso temporal ES/EN con el nombre de archivo y diagnóstico legible. El toast
  no captura teclado, permanece visible con la simulación pausada aunque el HUD
  técnico esté oculto, y la carga sólo sustituye la partida después de leerla
  correctamente. Las regresiones ECS cubren F5/F9, Ctrl+S/Ctrl+L, ambos fallos,
  preservación de estado/fuente, captura de la ventana de partidas y vencimiento
  del aviso; pasaron formato, Clippy estricto, 1.618 tests de cliente (2
  ignorados) y `sp1_playable_cycle` (2/2).
- [#577](https://github.com/cavazquez/openttdrs/issues/577) — smoke gráfico del
  paquete Linux y sesión de aceptación, completado con `4aef9c22` y el runtime
  `97f69b2a`. El [dry-run de release](https://github.com/cavazquez/openttdrs/actions/runs/35431897233)
  aprobó sus gates en Linux, macOS y Windows. El smoke actual comprueba el menú
  empaquetado y la presencia de sus assets desde un cwd aislado.

## Backlog ejecutable

**22 issues abiertos: 20 nuevos y los 2 de Snap.** El índice completo, los IDs
V1 y sus dependencias están en [acceptance-v1.md](acceptance-v1.md#contratos-pendientes-e-issues).
El [manifiesto](active-backlog.json) se contrastó con GitHub al preparar este
corte. Orden para implementar una tarea a la vez:

1. Gates que impedían falso verde: visual **#584** y sockets **#585** están
   implementados; cada commit conserva su evidencia local y debe obtener CI
   remota verde antes del cierre.
   El agregador **#603** puede prepararse con reportes sintéticos; su aceptación
   global depende de los contratos reales. Finalizar **#582/#583** con la
   evidencia restante, sin duplicar el fix local.
2. Contratos de datos: pagos **#586**, transferencia **#587**, ORDL **#588**,
   filtro de cargo GS **#598** y ownership **#591** están implementados y
   esperan CI remota vigente antes del cierre.
3. Recorridos: TransCargo **#596** y RoadHaul **#597** están certificados
   localmente con sus [fixtures deterministas](transcargo-v1.md) y
   [evidencia de pasajeros](roadhaul-v1.md), y esperan CI remota vigente.
   Siguen barco **#593**, avión **#594**, late join **#595** (después de
   #585), NewGRF/JSON **#592** y persistencia GS **#599** (después de #598).
4. Presentación y herramientas: raster Normal **#589** está implementado y
   certificado localmente en `1f170b07` (tres hashes por motor, presupuesto y
   cinco diagnósticos en [la evidencia Kale](raster-v1-kale.md)); espera CI
   remota vigente. Órdenes **#590** quedó certificada localmente en
   `c4251e4c` ([ocho perfiles y presupuesto fijo](orders-v1.md)); sigue editor
   **#600**.
5. Paquetes Windows **#601** y macOS **#602**. El [gate nativo ES/EN](package-native-graphical-v1.md)
   ya preserva paquete, SHA, logs y PNG desde cwd/perfil aislados; falta su
   dry-run en sesiones gráficas reales. Emitir con **#603** el reporte completo
   del SHA candidato; ninguna ausencia cuenta como aprobación.

Construcción básica, fixtures PBS y ruta vial/JSON ya tienen evidencia acotada
(95 tests seleccionados incluyendo FTA); se reutiliza y se reejecuta al
certificar otro candidato. No se crean issues duplicados de esas pruebas.

## Cómo trabajar y cerrar

- Una tarea en implementación a la vez. Cerrar al satisfacer su aceptación
  original, con commit y evidencia; no ampliar su contrato después.
- Los cuatro P0 de la alpha anterior se recuperaron; los gates V1 #584/#585
  separan ahora el falso éxito local, pero no se cierran sin CI remota verde.
  No llamar «CI verde» a un chequeo de binario si `--all-targets`, fuzz o el
  workflow siguen fallando o no se ejecutaron.
- Una tarea debe poder revisarse como un PR. Si aparecen dos causas
  independientes, dividir antes de continuar; el nuevo issue debe ser necesario
  para esta entrega, con reproducción y criterio de cierre propios.
- Usar tests de comportamiento para lógica y persistencia, ECS para interacción
  y el binario empaquetado para el recorrido visible. Los cambios de reglas
  nativas requieren un oracle OpenTTD apropiado. Una captura no certifica
  simulación ni compatibilidad SAV.
- Conservar las regresiones existentes. Un fallo ajeno se registra con alcance
  explícito; no se cambia un golden o una tolerancia para obtener verde.
- La evidencia técnica vive en su matriz o informe. El plan sólo enlaza el
  resultado; no se le agregan bitácoras de cada experimento.
- Los issues fuera del corte se cierran como **not planned**, nunca como prueba
  de implementación. Para reactivarlos hace falta un nuevo corte y una tarea
  concreta; un gap histórico no inicia automáticamente otro ciclo.

## Aceptación V1

Todos los contratos de cada fila del README ejecutados para el mismo SHA, con
los presupuestos de [acceptance-v1.md](acceptance-v1.md). Cero skips encubiertos,
umbrales ampliados o cierres `not planned` contados como cumplimiento. La
aceptación se publica por área; completar V1 no cierra paridad universal.

## Aceptación del corte alpha anterior

1. CI, plataformas, documentación y replay de fuzz verdes para el commit
   candidato, sin omitir pasos. Si aparece otro fallo, aislarlo antes de
   considerar listo el corte.
2. La ruta construida mediante comandos entrega y cobra antes de 40.000 ticks
   con replay idéntico; tras cargar JSON con carga a bordo, sus dos ramas
   continúan 2.000 ticks sin divergir y la recargada completa otra entrega.
3. Menú y guía ES/EN, confirmación visible de guardar/cargar y una sesión de
   15 minutos desde el paquete Linux: construcción → pago → guardar → cargar
   → nueva entrega. Conservar commit, configuración, reporte y artefactos.
4. Dry-run del workflow de release con smoke gráfico y gates existentes
   aprobados. La publicación posterior requiere un tag con la versión exacta y
   una revisión de sus artefactos.

**Resultado (2026-09-19):** los cuatro criterios se satisficieron. La
[CI de `aa8fe3d9`](https://github.com/cavazquez/openttdrs/actions/runs/35433985317),
plataformas y fuzz quedaron verdes; el dry-run de release y la sesión manual
del paquete quedan enlazados en #577. Después se creó el tag
[`v0.1.0-alpha.1`](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1),
el [workflow de release](https://github.com/cavazquez/openttdrs/actions/runs/35453383079)
publicó la prerelease y el Snap Store recibió la revisión 1 en
[`latest/edge`](https://snapcraft.io/openttdrs). La revisión se detectó después
como no arrancable bajo el mount read-only; la corrección queda acotada a
#582/#583 antes de publicar `0.1.0-alpha.2`.

El avance se mide por estos resultados y tareas aceptadas, no por cantidad de
commits, páginas de documentación o reducción de píxeles en una sola captura.

## Fuera de este corte

Paridad global Out2x/Out4x/Out8x, compositor universal, NewGRF completo, SAV
universal, matriz multiclima ilimitada, RNG runtime general, Oil Rig, oráculos
amplios road/rail, nuevos idiomas, Squirrel y red fuera de los dos clientes del
contrato V1. Las brechas y pruebas existentes se conservan en las matrices;
no se presentan como resueltas.

## Evidencia y antecedentes

- [Auditoría y disposición de los 15 issues anteriores](../audits/2026-09-18-direction.md).
- [Madurez y contratos técnicos](../PARIDAD.md).
- [Contratos, tolerancias y evidencia del corte V1](acceptance-v1.md).
- [Manifiesto del backlog](active-backlog.json), contrastado con GitHub al
  publicar este corte; el checker local valida consistencia documental, no
  consulta GitHub por sí mismo.
- [Historial anterior de 10.001 líneas](continuous-work-history-2026-09-18.md).

`AGENTS.md` conserva las reglas de trabajo y la autorización de publicación.
Su referencia a este archivo ahora resuelve a este plan. El backlog se ejecuta
de a una tarea atómica, con evidencia y documentación actualizada al cerrar.
