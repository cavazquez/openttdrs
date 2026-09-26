# Corte de controles y poses — 26/09/2026

Base: `c14e16d4`. Implementación autorizada de las siete divergencias de la
revisión local. No incluye nuevos atajos, cambios de preferencias ni release.

## Contratos atómicos

- BUILD-03 — completado: carretera sin fondos no modifica mapa/saldo/propiedad;
  preview y ejecución coinciden para presupuesto cero, exacto y costo menos
  uno, incluyendo autoslope. No prohíbe otros saldos negativos de simulación.
- INPUT-01 — completado: rueda/paneo pertenecen a UI o mapa, nunca ambos;
  modales, ventanas y listas no dejan pasar gestos de cámara.
- INPUT-02 — pendiente: arrastrar con derecho no rota ni cancela herramientas;
  conservar el clic contextual y diferenciarlo del arrastre en pantalla.
- BUILD-01 — pendiente: al soltar se usa el extremo actual, no el del frame
  anterior; cancelar y perder foco no confirman obras involuntarias.
- BUILD-02 — pendiente: construcción parcial informa teselas rechazadas y
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
La clasificación de clic frente a arrastre derecho queda en INPUT-02.
