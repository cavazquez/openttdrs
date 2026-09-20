# NewGRF V1 — CB36 de camión tras JSON

Contrato de [#592](https://github.com/cavazquez/openttdrs/issues/592). Certifica
un único camino reproducible de `NewGRF`; no afirma compatibilidad con GRFs
arbitrarios ni paridad general con OpenTTD.

## Fixture redistribuible

El test
[`v1_newgrf_cb36_json.rs`](../../crates/openttdrs-core/tests/v1_newgrf_cb36_json.rs)
construye de forma determinista e instala en un directorio temporal el
contenedor GRF V2 mínimo `v1-cb36-truck.grf`. Sus bytes son propios del
proyecto y se ceden al dominio público (`CC0-1.0`); no incorpora assets ni
código de terceros.

| Campo | Valor fijo |
|---|---|
| Nombre de archivo | `v1-cb36-truck.grf` |
| Contenedor | GRF V2 |
| GRFID | `43423336` (`CB36`) |
| Versión Action8 | `7` |
| Nombre Action8 | `V1 CB36 truck` |
| Parámetros del stack | `[]` |
| SHA-256 de bytes instalados | `7a1d02111ed80cb702d25367f48abb49b470eda074ce2d48189c89318e80163b` |
| Action0 vial | camión de carbón, capacidad 23, velocidad base `0x15 = 91` |
| Action2/3 | dispatch real por callback y propiedad; sólo CB36/`0x15` devuelve `37` |

El hash se verifica dentro del test antes de escribir el archivo, por lo que
un cambio en el builder no puede sustituir silenciosamente el fixture
certificado.

## Ejecución certificada

1. `scan_grf_bytes` inspecciona el contenedor y Action8 antes de instalarlo.
2. `apply_newgrf_vehicles_trains` carga el catálogo de producción desde ese
   archivo; no se inyecta un resolver Rust.
3. Sobre el escenario Temperate `first_route`, comandos públicos construyen
   carretera, depósito y paradas, compran el motor NewGRF, refitan carbón,
   fijan dos órdenes y arrancan el camión.
4. El Action2 real escribe `7C[3] = 37` mediante `\2psto`. CB36 para
   propiedad vial `0x15` devuelve `37`, distinto de la propiedad Action0
   `91`; los demás callbacks/propiedades devuelven `CALLBACK_FAILED` y
   preservan sus fallbacks.
5. La ruta continua y una copia `save_json` → `load_json` con recarga normal
   del mismo catálogo se avanzan 2.000 ticks. En cada tick se exige igualdad
   de hash canónico, velocidad efectiva, carga, órdenes y registros
   persistentes: cero diferencias permitidas.

Reproducción local:

```bash
cargo test --locked --offline -p openttdrs-core --test v1_newgrf_cb36_json -- --nocapture
```

Fuera del corte: parámetros distintos, otros CBIDs/propiedades, GRFs externos,
SAV universal y toda certificación visual de sprites NewGRF.
