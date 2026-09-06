# #426 — conservar columnas desconocidas al mutar structs anidados

Actualizado: **2026-09-06**. Sub-issue de [#328][parent]; no declara paridad
general de `.sav` ni de los pools nativos que todavía no tienen modelo.

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

El passthrough de tablas ya podía reencuadrar strings, listas escalares y
struct-lists de raíz. Cuando el campo conocido era un struct con columnas
adicionales dentro de cada elemento, una mutación de un scalar o de una lista
anidada trataba todo el rango raíz como incompatible y caía al writer canónico.
Ese fallback descartaba las columnas hermanas que sólo existían en el SAV
importado.

## Corrección acotada

- El descriptor del writer acepta un subconjunto compatible de subcampos de un
  struct importado; los campos desconocidos conservan su orden y bytes raw.
- La fusión recorre recursivamente los elementos del struct y permite que una
  string o lista escalar anidada cambie de longitud sin perder sus columnas
  hermanas.
- Una lista de structs que cambia de cantidad conserva el camino previo sólo
  cuando el descriptor completo es idéntico y el campo tiene longitud
  codificada; así se preservan las columnas desconocidas de la fila raíz.
- Si cambia la topología, el tipo, la identidad de filas o el descriptor de un
  campo que además cambia la cantidad, el resultado sigue siendo el writer
  canónico conservador.

## Regresiones

`passthrough_preserves_nested_unknown_column_when_scalar_and_list_change`
modifica simultáneamente `level` y la longitud de `values` dentro de un
struct, y verifica que `future` permanece en `0xCAFE`. La regresión
`passthrough_falls_back_when_nested_struct_count_changes` protege el fallback
para structs de tamaño fijo. Los tests nativos existentes de `CITY.supplied`,
`INDY.accepted` y `INDY.produced` siguen validando las listas que crecen con
schema idéntico.

Gates del corte:

```text
cargo test -p openttdrs-core --lib --quiet       # 2150 passed
cargo clippy -p openttdrs-core --all-targets -- -D warnings
git diff --check
./scripts/check_parity_docs_fresh.sh
```

## Pendiente real

Los cambios de cantidad de elementos con subcampos desconocidos dentro del
elemento, cambios de schema no compatibles, índices/filas y pools nativos no
modelados siguen usando el fallback canónico y continúan abiertos en #328.
