# V1-GS — objetivo de carbón y noticia única después de JSON

El contrato [#599](https://github.com/cavazquez/openttdrs/issues/599) fija una
sola meta GS-lite de carbón; no certifica GameScript/Squirrel ni un catálogo de
objetivos.

## Fixture acotado

El test `crates/openttdrs-core/tests/v1_gs_json.rs` reutiliza el recorrido
vial corto `truck_bay` y añade exactamente una segunda compañía, TransCargo.
TransCargo queda como compañía activa y, por ello, como destinataria de la
meta `CargoDelivered { Coal, min: 10 }`.

La mina contiene diez unidades iniciales y tiene producción posterior cero. El
camión de TransCargo se configura para carbón, conserva las órdenes públicas de
carga y descarga, y su capacidad se fija en cinco unidades. Una parada de
descarga cercana y una central eléctrica receptora permiten dos viajes físicos
de cinco unidades. El fixture no acredita el ledger, paquetes a bordo, pagos ni
noticias: la simulación los crea al cargar y entregar.

Al crear una rival, su límite efectivo de préstamo ahora toma el valor global
escalado vigente. Esto evita que `hydrate_runtime` modifique un `max_loan`
serializado de 300.000 a 540.000 y, por tanto, evita un hash distinto al
recargar una partida multicompañía.

## Ejecución y resultado

Desde la raíz del repositorio:

```sh
OPENTTDRS_SOURCE_SHA="$(git rev-parse HEAD)" \
  cargo test --locked --offline -p openttdrs-core --test v1_gs_json -- --nocapture
```

La ejecución fija informa:

```text
source_sha: <SHA pasado en OPENTTDRS_SOURCE_SHA>
save_tick: 1405
completion_tick: 2273
recipient: CompanyId(1)
delivered_at_save: 5
delivered_at_completion: 10
continuation_ticks: 2000
completion_news: 1
canonical_hash: 636218184125036548
```

La prueba guarda precisamente en 5/10, recarga una rama y avanza ambas ramas
2.000 ticks. En cada tick exige igualdad de destinataria activa, dos compañías,
ledger de carbón, meta, contador de noticias y hash canónico. Mientras el
camión lleva carbón pero aún no hay entrega final, exige que el progreso no
supere el ledger; así cargar no acredita progreso ficticio. Al llegar a 10/10
aparece una única noticia de finalización. Una segunda recarga y 16 ticks más
confirman que la marca persistente `victory_news_sent` no vuelve a publicarla.

## Límite explícito

No cubre intérprete Squirrel, edición de scripts, metas adicionales, rutas
alternativas, rivales controlados por IA ni compatibilidad universal de saves.
El cierre de #599 requiere además que el SHA publicado con esta prueba termine
sus checks remotos vigentes en verde.
