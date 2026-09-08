# PATS `game_creation.oil_refinery_limit` (#533)

Última actualización: 2026-09-08.

Sub-issue [#533](https://github.com/cavazquez/openttdrs/issues/533) del padre
[#328](https://github.com/cavazquez/openttdrs/issues/328).

## Alcance

El cargador importaba el resto de los ajustes de construcción, pero omitía
`game_creation.oil_refinery_limit`. La regla existente `CHECK_REFINERY`
utilizaba siempre 32 teselas, por lo que una partida con un límite distinto
tomaba decisiones de ubicación diferentes desde su siguiente intento.

El campo se conserva ahora en `ConstructionSettings`, compartido por JSON,
`SavGame` y `GameState`. El lector `PATS`/`OPTS` y el writer emplean el tipo
nativo `SLE_UINT8`, default 32 y rango `12..=128`. Modificar el valor luego de
importar actualiza la tabla exportada en vez de reutilizar el valor original.
Los JSON previos sin el campo adoptan 32.

El consumidor `CHECK_REFINERY` usa el límite guardado y lo escala por eje
cuando la dimensión supera 256. También respeta `freeform_edges`: el borde
norte tiene una tesela excluida sólo cuando esa opción está habilitada.

## Oracle y regresiones

Fuente local OpenTTD, commit `c2661164bcb6cbf5ab97b56ccbee7506a3b26833`:

- `src/table/settings/world_settings.ini:165`: tipo `SLE_UINT8`, desde
  `SLV_30`, default 32, mínimo 12 y máximo 128.
- `src/industry_cmd.cpp:1287`: `CheckScaledDistanceFromEdge` escala cada eje
  por separado; `CHECK_REFINERY` consulta `TileAddXY(tile, 1, 1)`.
- `src/map.cpp:246`: `DistanceFromEdgeDir` distingue los bordes libres en
  las direcciones NE/NW; SW/SE conservan siempre su fila de límite.

Las regresiones cubren default, extremos y clamp, precedencia PATS sobre
OPTS, tipo wire, hidratación, mutación después de importar y compatibilidad
JSON. El test de consumidor exporta e importa un mapa 512×256 con límite 48
y verifica estas fronteras calculadas desde las funciones nativas:

| Borde | `freeform_edges` | Admitido | Rechazado |
| --- | --- | --- | --- |
| NE | true | (95, 100) | (96, 100) |
| NE | false | (94, 100) | (95, 100) |
| NW | true | (200, 47) | (200, 48) |
| NW | false | (200, 46) | (200, 47) |
| SW | ambos | (414, 100) | (413, 100) |
| SE | ambos | (200, 206) | (200, 205) |

El mismo caso (95, 100) pasa con el límite importado 48 y se rechaza con
el antiguo default 32. Estas pruebas validan la decisión local de ubicación;
no equivalen a una nueva comparación de generación del mapa completo.

Validación focalizada: `cargo test -p openttdrs-core -j 2 refinery --lib --quiet`
completó 7 tests, sin fallos. `cargo fmt --all -- --check` y `git diff --check`
también pasaron.

## Reproducción del round-trip nativo

El fixture rico usa `oil_refinery_limit = 48`. El test opt-in exige ese valor
después de que OpenTTD cargue y vuelva a guardar la partida:

```bash
OPENTTDRS_DUMP_MVP_RICH_SAV=/tmp/oil-refinery-limit.sav \
  cargo test -p openttdrs-core -j 2 --lib \
  sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact

OPENTTDRS_REQUIRE_OPENTTD=1 \
OPENTTDRS_OTTD_ARTIFACT_DIR=/tmp/oil-refinery-artifacts \
  bash scripts/roundtrip_sav_openttd.sh /tmp/oil-refinery-limit.sav

OPENTTDRS_ROUNDTRIP_REQUIRE_OIL_REFINERY_LIMIT=1 \
OPENTTDRS_ROUNDTRIP_SAV=/tmp/oil-refinery-artifacts/oil-refinery-limit.resaved.sav \
  cargo test -p openttdrs-core -j 2 --test sav_openttd_roundtrip_subset \
  openttd_resaved_preserves_requested_oil_refinery_limit -- --exact
```

## Resultado de la comprobación nativa — 2026-09-08

La corrida real dedicated aceptó el SAV, abrió el socket en el puerto 3985
y confirmó `Map successfully saved`. El importador comprobó después el
valor 48 mediante el test opt-in, además del subconjunto general de la
fixture. No se contó el retorno temprano del test opcional como evidencia.

- SAV candidato: SHA-256
  `7678944e655909fc28355b86063faab48e8dda90c50ce17877b01e2514d72961`.
- SAV re-guardado por OpenTTD: SHA-256
  `1c398e64e8837e498a20a1fa6c53502a758691927cb8517b451a57a5de684033`.

El control de generación Temperate 64×64, seed `1330935378`, mantuvo
**0 teselas / 0 bloques 4×4** distintos en las seis fases
`landscape,clear,towns,industries,objects,trees`, además de RNG, pools e
intentos exactos. La corrida usó `generation_phase_parity.py --size 64
--seed 1330935378 --climate temperate --compact-report --require-exact`.
Es una regresión del default; el límite no-default se acredita mediante
round-trip nativo y las fronteras de ubicación anteriores.

## Límites

Este corte no conecta el ajuste a `CHECK_OIL_RIG`, no implementa el perímetro
de agua de las plataformas petrolíferas y no declara equivalencia del
constructor completo, del editor ni de los callbacks NewGRF de ubicación.
Esas diferencias siguen abiertas en [#531](https://github.com/cavazquez/openttdrs/issues/531)
y los padres correspondientes. #328 continúa abierto por los demás settings,
pools, órdenes y mutaciones de SAV pendientes.
