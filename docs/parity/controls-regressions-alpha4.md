# Corte de controles y poses — 26/09/2026

Base: `c14e16d4`. Implementación autorizada de las siete divergencias de la
revisión local. No incluye nuevos atajos, cambios de preferencias ni release.

## Contratos atómicos

- BUILD-03 — completado: carretera sin fondos no modifica mapa/saldo/propiedad;
  preview y ejecución coinciden para presupuesto cero, exacto y costo menos
  uno, incluyendo autoslope. No prohíbe otros saldos negativos de simulación.
- INPUT-01 — completado: rueda/paneo pertenecen a UI o mapa, nunca ambos;
  modales, ventanas y listas no dejan pasar gestos de cámara.
- INPUT-02 — completado: arrastrar con derecho no rota ni cancela herramientas;
  conservar el clic contextual y diferenciarlo del arrastre en pantalla.
- BUILD-01 — completado: al soltar se usa el extremo actual, no el del frame
  anterior; cancelar y perder foco no confirman obras involuntarias.
- BUILD-02 — completado: construcción parcial informa teselas rechazadas y
  causas, conserva las válidas y muestra el gasto real.
- POSE-01 — pendiente: avión usa posición física en todas las fases; igualdad
  a alpha cero y continuidad entre ticks sin modificar simulación.
- POSE-02 — pendiente: barco interpola tesela/subtesela coherentemente; no
  salta 16 píxeles al cruzar un umbral de alpha.

Para input y comandos, cero efectos no autorizados y cero diferencias
discretas. Para continuidad flotante, tolerancia 0,001 unidades de mundo en
barridos de alpha casi idénticos; comprobar zooms 0,12/0,25/0,5/1.
Se preservan los contratos V1 existentes: no constituye paridad global.

## Evidencia por etapa

Se agrega al validar cada cambio, sin contar tests pendientes como aprobados.

### BUILD-03

El presupuesto se verifica antes de mutar, sumando el costo de autoslope
cuando corresponde. El chequeo es compartido con el preview para PlaceRoad,
PlaceRoadBits, SetRoadBits y PlaceTramBits. Se conservan conexiones internas
de depósitos sin doble cobro. Referencia de regla: OpenTTD 15.3,
`command.cpp::InternalExecuteValidateTestAndPrepExec`, CheckCompanyHasMoney
antes de ejecutar; no se declara equivalencia universal de precios viales.

La regresión falló primero con presupuesto cero. Ahora aprueban 24 casos
(cuatro comandos × plano/pendiente × cero/costo-1/costo), con snapshot JSON
idéntico antes/después de cada rechazo y preview sin mutaciones. El test
de cliente da fondos para una sola tesela de tres: saldo final cero, una
construida y dos intactas. El aviso de obra parcial pertenece a BUILD-02.

Validación: suites completas core y cliente (1.639 aprobados, dos ignores
preexistentes en cliente), Clippy core/cliente --all-targets -D warnings,
formato, checker documental y diff. No se cambió ningún golden ni presupuesto.

### INPUT-01

La política compartida PointerCapture bloquea rueda y paneo de cámara sobre
UI interactiva, minimapa visible, guardado, consola y modales. No consume
los eventos de rueda: las listas conservan su desplazamiento. Nodos ocultos
no retienen captura. La referencia conceptual es el despacho al viewport o
ventana bajo el puntero en OpenTTD 15.3 (`window.cpp`, DispatchMouseWheelEvent).

Se conservaron como regresiones los diagnósticos de la auditoría: un modal
o UI bajo el mouse antes cambiaban escala de 1 a 0,5. Ahora la cámara queda
idéntica en los seis zooms; además, la prueba con ClassicScrollViewport y un
MouseWheel real avanza exactamente un paso sin mover ni ampliar el mapa.
Guardado/minimapa capturan aunque no haya un botón hovered; ocultar UI libera
la rueda. Cuatro tests nuevos, suite cliente 1.643 aprobados/dos ignores,
Clippy --all-targets -D warnings, formato, docs y diff aprobados.

### BUILD-02

DragBuildReport conserva cada rechazo con coordenada/causa, acciones aceptadas
y diferencia real de saldo. El toast no desaparece porque otra tesela sí se
construyó: muestra conteos, gasto neto y hasta tres rechazos para mantenerlo
legible. Comandos de área cuentan como una acción, no como múltiples teselas
supuestamente construidas. Cliente de red muestra solicitudes pendientes,
no construcción ni costo confirmado. La política sigue siendo parcial.

Regresiones de carretera/tranvía/vía con agua en medio: dos acciones aplicadas,
un rechazo, coordenada (3,3), causa y costo exactos. Presupuesto para una sola
tesela conserva ambos rechazos restantes; presupuesto cero conserva el
snapshot normalizado. El test ECS de confirmación verifica el mensaje final.

La prueba integrada de señales reveló densidad aplicada dos veces (preview
y ejecución): ahora se ejecuta el path completo con un solo muestreo. Preview
y resultado coinciden exactamente en [1,5,9], densidad cuatro.
Suite cliente: 1.654 aprobados/dos ignores; Clippy --all-targets -D warnings,
formato, docs y diff aprobados. No se modificaron reglas de simulación.
La clasificación de clic frente a arrastre derecho queda en INPUT-02.

### INPUT-02

Un clasificador compartido mantiene el dueño del gesto derecho desde presión
hasta liberación. A partir de cuatro píxeles recorridos se considera paneo;
el clic contextual se emite una sola vez al soltar. Abrir un modal, perder
foco o salir de la ventana cancela el gesto. Comenzar sobre UI no permite
continuarlo sobre el mapa. Se conserva la rotación contextual del cliente;
no se declara que este binding sea idéntico a OpenTTD.

Cuatro regresiones ECS ejecutan clasificación, rotación y cámara: seis zooms,
jitter, ida/vuelta, pérdida de foco, modal y origen UI. Paneo conserva la
herramienta y el drag de construcción. Suite cliente: 1.647 aprobados, dos
ignores; Clippy --all-targets -D warnings, formato, docs y diff aprobados.

### BUILD-01

ConfirmDrag lleva el extremo actual y reconstruye la selección antes de
aplicar comandos. Fuera del mapa conserva explícitamente el último tramo
válido, sin simular un cursor en (0,0). UI, pérdida de foco/cursor y una
liberación perdida cancelan y limpian el drag; una confirmación tardía queda
sin efecto. El contrato local es la selección visible al soltar, relacionado
con OnPlaceMouseUp/selend en OpenTTD 15.3; no se cambia su geometría.

Tres regresiones: presión (1,3), frame anterior (2,3), liberación (5,3)
construye las cinco teselas; liberación fuera de mapa conserva sólo las dos
válidas; cinco interrupciones dejan snapshot JSON idéntico incluso después
de una confirmación tardía. Suite cliente 1.650 aprobados/dos ignores,
Clippy --all-targets -D warnings, formato, docs y diff aprobados.
