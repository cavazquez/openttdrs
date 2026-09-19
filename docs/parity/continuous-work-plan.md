# Corte cerrado — primera ruta jugable

Actualizado: **2026-09-19**. Base auditada: `ca698e50`.

La instrucción del usuario del 2026-09-18 reemplaza el orden anterior de
perseguir paridad por subsistemas. El objetivo inmediato es entregar una
experiencia acotada que pueda jugarse y verificarse de principio a fin.
La paridad con OpenTTD sigue siendo una referencia técnica de largo plazo.

La entrega está publicada como
[prerelease `v0.1.0-alpha.1`](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
y [Snap Store `latest/edge`](https://snapcraft.io/openttdrs) (revisión 1,
Linux amd64). No convierte las brechas históricas de paridad en tareas activas.

## Entrega comprometida (cumplida)

Desde el menú, abrir **Primera ruta**, construir un servicio de camiones
entre una mina de carbón y una central eléctrica, recibir el primer pago,
guardar JSON, cargar y completar otra entrega. El paquete Linux debe funcionar
fuera del checkout. Escenario Temperate plano 64×64, año 1950, una compañía,
OpenGFX, sin NewGRF, IA, desastres ni averías.

Se reutilizan comandos, simulación, UI, persistencia y empaquetado existentes.
Demo, Nueva partida y las demás capacidades permanecen disponibles; esta
entrega sólo certifica el recorrido descrito. No requiere paridad raster
perfecta ni equivalencia universal de saves o generación.

## Entrega publicada desde el corte

- Distribución alpha — el tag
  [`v0.1.0-alpha.1`](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
  produjo los paquetes de GitHub para Linux x86_64, Windows x86_64 y macOS
  arm64. El Snap estricto `core24` se compiló con la misma versión, pasó los
  linters de Snapcraft con sólo avisos de bibliotecas X11 preventivas y se
  publicó como revisión 1 de `latest/edge`. El launcher y los assets incluidos
  pasaron `--check-assets` desde el árbol empaquetado.
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
- [#572](https://github.com/cavazquez/openttdrs/issues/572) — escenario
  `first_route` y ruta de carbón construida sólo mediante comandos, cerrado con
  `53d727c9`; la entrega, el pago y el hash canónico se verifican dos veces y
  el estado posterior conserva ese hash al serializar y recuperar JSON.
- [#573](https://github.com/cavazquez/openttdrs/issues/573) — continuación
  productiva de esa ruta tras guardar JSON, cerrada con `d55686ae`; con carga
  a bordo, ambas ramas conservan hash, vehículo, órdenes, carga, stocks,
  ingresos y RNG durante 2.000 ticks, y la recargada vuelve a entregar y cobrar.
- [#574](https://github.com/cavazquez/openttdrs/issues/574) — entrada
  localizada «Primera ruta»/“First route” desde el menú, cerrada con
  `64ee70db`; abre el escenario vacío de infraestructura y vehículos, pausado
  y enfocado entre mina y central. La regresión ECS cubre dos entradas y la
  limpieza entre sesiones; el binario real produjo capturas ES/EN a 1280×720
  bajo Weston headless.
- [#575](https://github.com/cavazquez/openttdrs/issues/575) — guía visible de
  objetivo y avance, cerrada con `ff1b4730`; el panel específico de la sesión
  deriva carretera, paradas/depósito, camión a carbón, órdenes, marcha y el
  hito final de `GameState`. Éste exige que la central haya aceptado carbón y
  que la compañía activa haya cobrado; los tests recorren estado incompleto,
  entrega real, ausencia de ingreso/recepción y reconstrucción tras JSON. El
  binario real produjo capturas ES/EN a 1280×720 del panel cargado.
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
  aprobó sus gates en Linux, macOS y Windows, incluido el smoke gráfico Linux.
  La sesión manual del paquete aislado recorrió construcción, primera entrega,
  F5, F9, recarga y una entrega posterior: 2 entregas, 28 unidades de carbón
  cargadas/entregadas e ingreso 1064, sin errores ECS ni de despawn.

## Backlog ejecutable

El corte no tiene tareas ejecutables abiertas. Cada trabajo futuro debe volver a
pasar por una auditoría de dirección y abrir un issue independiente, con
alcance, aceptación, dependencias y exclusiones propios; no se reactivan épicas
históricas como tareas por defecto.

## Cómo trabajar y cerrar

- Una tarea en implementación a la vez. Cerrar al satisfacer su aceptación
  original, con commit y evidencia; no ampliar su contrato después.
- Los cuatro gates P0 ya están recuperados. No llamar «CI verde» a un chequeo
  de binario si `--all-targets`, fuzz o el workflow siguen fallando.
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

## Aceptación del corte

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
[`latest/edge`](https://snapcraft.io/openttdrs).

El avance se mide por estos resultados y tareas aceptadas, no por cantidad de
commits, páginas de documentación o reducción de píxeles en una sola captura.

## Fuera de este corte

Paridad global Out4x/Out8x, compositor universal, NewGRF completo, SAV universal,
matriz multiclima ilimitada, RNG runtime general, Oil Rig, oráculos amplios
road/rail, nuevos idiomas y expansión de multiplayer. Las brechas y pruebas
existentes se conservan en las matrices; no se presentan como resueltas.

## Evidencia y antecedentes

- [Auditoría y disposición de los 15 issues anteriores](../audits/2026-09-18-direction.md).
- [Madurez y contratos técnicos](../PARIDAD.md).
- [Manifiesto del backlog](active-backlog.json), contrastado con GitHub al
  publicar este corte; el checker local valida consistencia documental, no
  consulta GitHub por sí mismo.
- [Historial anterior de 10.001 líneas](continuous-work-history-2026-09-18.md).

`AGENTS.md` conserva las reglas de trabajo y la autorización de publicación.
Su referencia a este archivo ahora resuelve a este plan. El backlog se ejecuta
de a una tarea atómica, con evidencia y documentación actualizada al cerrar.
