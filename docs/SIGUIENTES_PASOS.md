# Siguientes pasos — openttdrs

Documento vivo: **qué documentar**, **dónde está**, y **cómo seguir** el desarrollo después
de I0–I5 y del renderer isométrico con mapas reales.

---

## Índice de documentación técnica

| Documento | Contenido |
|-----------|-----------|
| [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) | Filosofía por incrementos, spec I0–I8, **estado actual** del repo. |
| [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md) | NFO, transparencia 8bpp, IDs de sprite, proyección isométrica, tabla road_tx/ty. |
| [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) | MAPT, `m5` (carretera normal / cruce / depósito), `.ottdmap`, relieve 8px, referencias upstream. |
| [INFORME_ARQUITECTURA_OPENTTD.md](INFORME_ARQUITECTURA_OPENTTD.md) | Visión del código de referencia en `reference/openttd-upstream/`. |

---

## Hallazgos ya fijados en código y docs

1. **Sprite “coal mine” equivocado** — el ID antiguo era de otra industria; headframe correcto
   y proceso de verificación: `SPRITES_OPENGFX.md` + commits históricos.
2. **Cruces a nivel** — no usar bits 0–3 de `m5` como road bits; el eje de la carretera va
   en el bit 0. Detalle en `TILES_Y_SAVEGAMES_OPENTTD.md`.
3. **MAPT + `m5`** — hace falta el byte MAPT crudo en `Tile` para decodificar túneles/puentes
   frente a `MP_ROAD`.
4. **Intercambio `road_tx` ↔ `road_ty`** — respecto a `RoadDir`, para alinear textura OpenGFX
   con la isometría del cliente; **validado visualmente** en mapas `.ottdmap`.
5. **Limitaciones actuales** — solo tres sprites de carretera (tramo X, Y, cruce): esquinas y
   T reales del original son aproximaciones; tranvía en `m3` no está en el `.ottdmap`.
6. **Industrias sandbox** — `PlaceIndustrySpec` usa layouts base de OpenTTD para mina de carbón,
   fábrica, granja, bosque, refinería, pozos, aserradero y minas. El ghost de construcción consume
   la misma plantilla (`openttdrs_core::industry_template`), así que el footprint previsto y el
   construido comparten una única fuente de verdad. Refinería quedó visualmente muy cercana al
   original; `Farm`, `Factory` y `Coal Mine` son aceptables, pero podrían mejorar con calibración
   fina de offsets/capas.
7. **Fuente UI UTF-8** — `static/fonts/DejaVuSansMono.ttf` es un asset versionado separado de
   `assets/`; se usa para que Bevy renderice acentos en paneles (`Fábrica`, `Refinería`,
   `Petróleo`). `assets/` queda reservado para gráficos/sonidos generados por scripts e ignorados.

## Estado del refactor del cliente

- `main.rs` conserva el armado de la app y los sistemas principales, pero el render de mapa vive
  en `crates/openttdrs-client/src/render/`.
- El render/índice de vehículos quedó aislado en `vehicle_render.rs`, separando esa lógica de la
  construcción de teselas.
- Las variables de entorno del cliente se leen desde `config.rs` para evitar parsing duplicado.
- `RenderGrid` tiene tests para la inferencia de costa en agua exportada con `m5=0`.

---

## Cómo seguir (prioridades sugeridas)

Orden **no estricto**: depende de si querés **parecerse más al original**, **jugabilidad**, o
**ingeniería**.

### A. Visual y fidelidad al mapa (sin tocar I6)

- Extraer y usar **sprites de esquina / T** de OpenGFX (`GetRoadSpriteOffset` en upstream) o
  ampliar `descargar_graficos.sh`.
- **Vías**: sprites de rail por `trackbits` (similar a road bits).
- **Casas / estaciones**: sustituir tintes planos por sprites o conjuntos mínimos.
- **Industrias**: si se busca paridad visual más estricta, calibrar `w/h/xrel/yrel` en
  `industry_gfx_data_generated.rs` para los sprites que aún usan el fallback genérico
  `64x48/-32/-32`, y revisar el orden Z/capas de `Farm`, `Factory` y `Coal Mine`.
- **Agua**: animación o variante de sprite si molesta el aspecto “plano”.
- **`.ottdmap`**: el bloque denso MAP1 v1 **ya incluye** planos `m3`/`m3hi` (`parse_sav.py` los exporta). Lo pendiente suele ser **usar** `m3` en el render de carretera/tranvía y más fixtures de prueba; ver `docs/OTTDMAP_FORMAT.md` y `crates/openttdrs-core/tests/ottdmap_m3_road_fixture.rs`.
- **Rendimiento**: culling por frustum o LOD en mapas 256×256+.

### B. Cadena incremental formal (gameplay)

- **I6 — Comandos del jugador** — hecho: `openttdrs_core::command`, clics en cliente.
- **I7 — Save/load** — hecho: `openttdrs_core::save` (JSON con `version` + `state`, carga de legado plano); atajos F5/Ctrl+S, F9/Ctrl+L.
- **I8 — Red** mínima con log de comandos (pendiente).

### C. Higiene y referencia

- Mantener `reference/openttd-upstream/` actualizado (`scripts/fetch-openttd-reference.sh`).
- Tests que carguen un `.ottdmap` pequeño en memoria y comprueben `effective_road_bits` /
  dimensiones (opcional, sin Bevy).
- Seguir achicando `main.rs`: buenos candidatos son hotkeys/save-load, debug gizmos y animación
  de agua.

---

## Comandos útiles

```bash
# Mapa real
python3 scripts/parse_sav.py partida.sav assets/maps/mapa.ottdmap
OTTDMAP_FILE=assets/maps/mapa.ottdmap cargo run -p openttdrs-client

# Solo demo procedural
cargo run -p openttdrs-client

# Tests core
cargo test -p openttdrs-core

# Validar panel/ghost de industrias
cargo test -p openttdrs-client industry
cargo test -p openttdrs-client preview
```

---

## Si algo se pierde otra vez

1. Buscar en los cuatro MD de `docs/` anteriores.
2. Mirar comentarios en `crates/openttdrs-client/src/main.rs` (`effective_road_bits`, sprites).
3. Upstream: `road_map.h`, `road_func.h`, `tile_map.h`, `saveload/map_sl.cpp`.
