# #382 — preservar PATS.order.selectgoods ejecutado por el core

Actualizado: **2026-09-05**. Sub-issue de [#328][parent].

[parent]: https://github.com/cavazquez/openttdrs/issues/328

## Divergencia medida

OpenTTD 15.3 guarda OrderSettings::selectgoods como PATS.order.selectgoods.
El valor decide si MoveGoodsToStation puede enviar carga a una estación que
ningún vehículo intentó cargar todavía. Aunque openttdrs ya ejecutaba ese gate
en GameState.order, el importador ignoraba la columna y el writer no la
emitía. Una partida con el valor false cambiaba a true al importarse, y una
mutación podía quedar oculta detrás del passthrough de PATS.

## Corrección acotada

ParsedSettings decodifica el bool desde PATS y el fallback OPTS; SavGame lo
transporta hasta GameState.order.selectgoods. El writer declara la columna
SLE_BOOL nativa y la incluye en su snapshot semántico: por ello un PATS
importado sin cambios conserva su cuerpo original, mientras una modificación
de selectgoods parchea esa columna o cae de forma segura al encoder canónico.

El alcance es el setting ejecutado por el core. No afirma que todas las
opciones PATS/OPTS, los modos de carga avanzados ni la configuración de UI
tengan paridad completa.

## Regresiones y oracle

Las regresiones cubren:

- lectura de PATS y de OPTS;
- wire SLE_BOOL, round-trip SavGame y rehidratación de GameState;
- el efecto observable: con selectgoods false, una parada de camión sin visita
  previa puede recibir carbón;
- una mutación después de importar reescribe el valor, en vez de reutilizar el
  PATS antiguo;
- el fixture rico no-default y la relectura tras OpenTTD dedicated.

El oracle es OpenTTD 15.3, commit
c2661164bcb6cbf5ab97b56ccbee7506a3b26833:

- src/table/settings/game_settings.ini, entrada order.selectgoods;
- build/generated/table/settings.h, descriptor SDT_BOOL;
- src/settings_type.h, OrderSettings;
- src/station_cmd.cpp, consumidor de la política al mover carga.

Artefactos de la corrida final:

- candidato Rust: b28b8e4dbc42f75009d3bab5df10d04becce791ea42843e2de2fd6c0e6c1ce3b;
- re-guardado por OpenTTD: 7374e01bf335ee233c46f3e5714aca1f16a00d8fc1eb0039cf528d6eb784f20b.

La reproducción dedicada es:

    selectgoods_artifacts=$(mktemp -d /tmp/openttdrs-sav-selectgoods.XXXXXX)
    OPENTTDRS_DUMP_MVP_RICH_SAV="$selectgoods_artifacts/selectgoods.sav" \
      cargo test -p openttdrs-core --lib \
      sav::write::tests::export_mvp_rich_emits_indy_road_vehs_and_stations -- --exact
    OPENTTDRS_REQUIRE_OPENTTD=1 OPENTTD_SMOKE_PORT=3999 \
      OPENTTDRS_OTTD_ARTIFACT_DIR="$selectgoods_artifacts/artifacts" \
      OPENTTDRS_OTTD_LOG_DIR="$selectgoods_artifacts/logs" \
      bash scripts/roundtrip_sav_openttd.sh "$selectgoods_artifacts/selectgoods.sav"
    OPENTTDRS_ROUNDTRIP_REQUIRE_SELECTGOODS=1 \
      OPENTTDRS_ROUNDTRIP_SAV="$selectgoods_artifacts/artifacts/selectgoods.resaved.sav" \
      cargo test -p openttdrs-core --test sav_openttd_roundtrip_subset \
      openttd_resaved_preserves_requested_selectgoods -- --exact

## Pendiente real

Siguen fuera del corte las demás columnas de PATS/OPTS que no tienen modelo
ejecutable, además de improved/gradual loading, settings de estación y los
otros pools y callbacks pendientes de #328.
