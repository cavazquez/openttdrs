# Flujo: savegame → mapa → cliente → persistencia

Guía única que enlaza el pipeline principal del repo. Para detalle binario de teselas y chunks, ver **[TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md)**. Para diseño incremental del proyecto, ver **[DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md)**.

## 1. De `.sav` a `.ottdmap`

1. Tener un save de OpenTTD (`.sav` o comprimido OTTZ/OTTX según soporte en `scripts/parse_sav.py`).
2. Ejecutar:
   ```bash
   python3 scripts/parse_sav.py ruta/partida.sav salida.ottdmap
   ```
3. El binario `MAPO` incluye planos densos v5/v5+12 y footers opcionales (**INDP**, **STNN**, **TNBP**, **STXY**). Ver doc de teselas para el layout exacto.

**CI:** el golden se valida con `python3 scripts/verify_parse_sav_reference.py` (fixture en `tests/fixtures/`).

## 2. Cliente con mapa real

1. Generar assets OpenGFX: `./scripts/descargar_graficos.sh --8bpp` (requiere `grfcodec`; salida bajo `assets/`, ignorada por git).
2. Arrancar:
   ```bash
   OTTDMAP_FILE=salida.ottdmap cargo run -p openttdrs-client
   ```

## 3. Simulación y persistencia JSON

- El núcleo expone `GameState::save_json` / `load_json`.
- **Arranque desde JSON:** `OTTDJSON_LOAD=estado.json cargo run -p openttdrs-client`.
- **En ventana:** **F5** guarda, **F9** carga y **redibuja** suelo/vías/vehículos (también si cambia el tamaño del mapa). Ruta por defecto `openttdrs_sim.json` o variable `OPENTTDRS_JSON_SAVE` al arranque; **F4** alterna entre `openttdrs_sim.json` y `openttdrs_autosave.json`.
- **P** pausa el avance de ticks de simulación.

## 4. Qué simula el core hoy

- Industrias: varios `IndustryKind` (mina, bosque, pozo de petróleo, fábrica con producción más lenta); vehículos **camión** y **tren** (misma lógica de movimiento; el tren conviene asociarlo a rutas con vía).
- Footers **STNN** / **TNBP** en `OttdmapExtras`; **TNBP**: decode Sl/gamma (`tnbp_decode`), JSON (`tnbp_blob_to_json_value` / `OttdmapExtras::tnbp_json_summary`), túneles JGR en `GameState::jgr_tunnels_from_footer`, cruce con mapa (`Map::jgr_tunnel_endpoint_match_stats`). Fixture `tests/fixtures/v5p12_tnbp.ottdmap`; regenerar con `scripts/gen_tnbp_fixture_ottdmap.py`. Saves reales: `parse_sav.py` + `OTTDMAP_FILE`; depuración `OTTDMAP_TNBP_JSON=1`. Validación CLI de un `.sav` → `.ottdmap` → resumen TNBP: `scripts/validate_sav_tnbp.sh partida.sav` (o `cargo run -p openttdrs-core --example validate_ottdmap_tnbp -- mapa.ottdmap`).

## 5. Enlaces rápidos

| Documento | Contenido |
|-----------|-----------|
| [README.md](../README.md) | Cómo correr, CI, stack |
| [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) | MAPT, planos, footers, OpenTTD vs export |
| [SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md](SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md) | Notas de una sesión de implementación v5+12 / señales |
