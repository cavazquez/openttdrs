# Objects CB15D: autoslope durante terraform

## Estado

Implementado en `main` el 2026-09-06. El issue [#427](https://github.com/cavazquez/openttdrs/issues/427) queda limitado al call site de `TerraformTile_Object`; los callbacks de texto y los scopes todavía no modelados continúan en el parent NewGRF.

## Contrato upstream

OpenTTD consulta `CBID_OBJECT_AUTOSLOPE` (`0x15D`) cuando una operación de terraform afecta una tesela de un objeto existente. La consulta recibe el contexto de la instancia `Object` y el pueblo parent. El resultado es booleano: `CALLBACK_FAILED` o cero permiten la operación; un resultado no nulo la rechaza.

El motor también conserva las guardas de `TerraformTile_Object`: la pendiente vieja y nueva no pueden ser empinadas y el `TileMaxZ` absoluto debe permanecer igual. Si no se puede resolver la instancia, el catálogo o la topología de la huella, la implementación rechaza de forma conservadora antes de mutar el mapa.

## Implementación

- `CBID_OBJECT_AUTOSLOPE` y la máscara Action0 `0x15D` están expuestos en el catálogo público.
- `raise_land`, `lower_land` y `level_land` habilitan el preflight de objetos y evalúan CB15D después de validar dinero, industria y geometría.
- El resolver reutiliza `resolve_object_animation_callback`, por lo que conserva los scopes Object/Town, los parámetros GRF y el writeback parent aislado del preflight.
- El rechazo ocurre antes de aplicar el delta; las órdenes fallidas no cambian teselas, objetos, pueblos ni fondos.

## Regresiones y límites

Las regresiones sintéticas cubren resultado cero (permite), resultado no nulo (rechaza sin mutación) y la semántica común del resolver. El fallback sin callback conserva el comportamiento vanilla. Quedan fuera los strings de error específicos de GRF, el writeback `7C` propio de la tesela/instancia, scopes/vecinos adicionales y la limpieza automática de topologías que OpenTTD puede resolver fuera de esta ruta.
