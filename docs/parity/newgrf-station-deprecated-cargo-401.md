# NewGRF Station deprecated cargo variables — corte #401

Actualizado: 2026-09-06.

## Alcance

OpenTTD conserva una familia legacy de variables directas en el scope de
estación: `0x8C..0xEC`, ocho subvariables por cada una de las doce ranuras
nativas. El corte reproduce el índice `GB(variable - 0x8C, 3, 4)` y publica:

1. total en espera;
2. nibble bajo del total acotado a 4095 y bit `Acceptance` en `0x80`;
3. días desde la última recogida;
4. rating;
5. primer `StationID`;
6. períodos de tránsito;
7. última velocidad;
8. edad de la última carga.

Los valores viven en `Action2EvalCtx.vars`, por lo que la API legacy sin
tesela y el resolver map-aware comparten la respuesta. El `StationID` sólo se
devuelve cuando el packet conserva como origen la propia estación y el modelo
conoce su ID nativo; en los demás casos se conserva `StationID::Invalid()`.

## Regresiones y límite

La prueba cubre datos no nulos de carbón, aceptación, límite de 12 bits,
períodos, rating, velocidad/edad y el mismo resultado en ambos contextos.
Las variables parametrizadas modernas `0x60..0x69`, cargos custom traducidos
por CTT, textos/sonidos y el resto del scope `BaseStation` siguen siendo
áreas independientes del padre #329.
