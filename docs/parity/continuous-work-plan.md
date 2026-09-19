# Plan vigente — primera ruta jugable

Actualizado: **2026-09-18**. Base auditada: `ca698e50`.

La instrucción del usuario del 2026-09-18 reemplaza el orden anterior de
perseguir paridad por subsistemas. El objetivo inmediato es entregar una
experiencia acotada que pueda jugarse y verificarse de principio a fin.
La paridad con OpenTTD sigue siendo una referencia técnica de largo plazo.

## Entrega comprometida

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

## Backlog ejecutable

Cada fila es un resultado independiente. El issue contiene alcance, aceptación,
dependencias y exclusiones; no se mantienen épicas abiertas como tareas.
Las cinco tareas pendientes están agrupadas en el hito existente
[0.1 — vertical slice](https://github.com/cavazquez/openttdrs/milestone/1).

| Orden | Issue | Entrega | Dependencia funcional |
|---:|---|---|---|
| 1 | [#573](https://github.com/cavazquez/openttdrs/issues/573) | Continuación de la ruta después de guardar JSON | #572 ✅ |
| 2 | [#574](https://github.com/cavazquez/openttdrs/issues/574) | Entrada «Primera ruta» en el menú | #572 ✅ |
| 3 | [#575](https://github.com/cavazquez/openttdrs/issues/575) | Objetivo y avance visibles de esa ruta | #573, #574 |
| 4 | [#576](https://github.com/cavazquez/openttdrs/issues/576) | Confirmación y errores visibles de F5/F9 | — |
| 5 | [#577](https://github.com/cavazquez/openttdrs/issues/577) | Smoke gráfico del paquete Linux y sesión de aceptación | #572 ✅, #573–#576 |

**Próxima implementación: #573.** La reorganización de issues y documentación
no cuenta como implementación de las cinco tareas pendientes.

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
2. La ruta construida mediante comandos ya entrega y cobra antes de 40.000
   ticks con replay idéntico. Falta que #573 pruebe que, tras cargar JSON con
   carga a bordo, 2.000 ticks coinciden con la rama continua y completan otra
   entrega.
3. Menú y guía ES/EN, confirmación visible de guardar/cargar y una sesión de
   15 minutos desde el paquete Linux: construcción → pago → guardar → cargar
   → nueva entrega. Conservar commit, configuración, reporte y artefactos.
4. Dry-run del workflow de release con smoke gráfico y gates existentes
   aprobados. La publicación de una release es una decisión posterior; no se
   etiqueta una versión durante esta reorganización.

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
Su referencia a este archivo ahora resuelve a este plan. El pedido actual es
auditar y reorganizar; no ejecutar anticipadamente el nuevo backlog.
