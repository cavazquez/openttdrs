# openttdrs

Port **incremental** de ideas y mecánicas inspiradas en [OpenTTD](https://www.openttd.org/) hacia **Rust**, con motor gráfico [Bevy](https://bevyengine.org/). El objetivo a largo plazo es un simulador modular; la **paridad total** (NewGRF, red, saves idénticos) es un alcance opcional y costoso en tiempo.

> **Rendimiento en tu máquina:** compilar Bevy y dependencias puede ser pesado. Si notas saturación de CPU o RAM, usa por ejemplo `cargo build -j 1` o deja que el flujo de [CI](.github/workflows/ci.yml) valide el build en GitHub Actions.

**Roadmap:** hito [0.1 — vertical slice](https://github.com/cavazquez/openttdrs/milestone/1) con **8 incrementos** (I1–I8, issues [#14](https://github.com/cavazquez/openttdrs/issues/14)–[#21](https://github.com/cavazquez/openttdrs/issues/21)). Spec detallada en [docs/DISENO_INCREMENTAL.md](docs/DISENO_INCREMENTAL.md); correspondencia con el código C++ del upstream en [docs/INFORME_ARQUITECTURA_OPENTTD.md](docs/INFORME_ARQUITECTURA_OPENTTD.md).

**Flujo de trabajo** (save → mapa → cliente → JSON): [docs/FLUJO_MAPA_Y_CLIENTE.md](docs/FLUJO_MAPA_Y_CLIENTE.md).

---

## Stack tecnológico

| Tecnología | Descripción |
|------------|-------------|
| 🦀 [Rust](https://www.rust-lang.org/) | Lenguaje y toolchain; edición **2024** en el workspace. |
| 📦 [Cargo](https://doc.rust-lang.org/cargo/) | Workspace con crates `openttdrs-core` y `openttdrs-client`. |
| 🎮 [Bevy](https://bevyengine.org/) | Motor ECS, ventana, cámara 2D, gizmos de depuración (cliente). |
| 🖼️ [wgpu](https://wgpu.rs/) (vía Bevy) | API gráfica usada por debajo del render de Bevy. |
| 🧪 Tests + clippy | `cargo test` en el workspace; golden `parse_sav` en Python; CI en GitHub. |
| ✅ [GitHub Actions](https://docs.github.com/en/actions) | Workflow `ci.yml`: `fmt`, `clippy`, `test`, golden `parse_sav`, `py_compile` scripts, `build`. |
| 🤖 [Dependabot](https://docs.github.com/en/code-security/dependabot) | Actualizaciones **mensuales** de Cargo y Actions (`.github/dependabot.yml`). |
| 📚 OpenTTD upstream | Solo referencia local; ver sección siguiente. |

**MSRV:** el workspace declara `rust-version` alineado con [Bevy 0.18.1](https://crates.io/crates/bevy) (consulta el `Cargo.toml` raíz). `rust-toolchain.toml` usa el canal `stable` con `rustfmt` y `clippy`.

---

## Código de referencia OpenTTD (no versionado)

El repositorio oficial es [OpenTTD/OpenTTD](https://github.com/OpenTTD/OpenTTD) (GPL-2.0). El clon local se ignora en git (`.gitignore`) para no inflar el historial.

```bash
./scripts/fetch-openttd-reference.sh
```

Equivale a un `git clone --depth 1` bajo `reference/openttd-upstream/`. El análisis de módulos está en [docs/INFORME_ARQUITECTURA_OPENTTD.md](docs/INFORME_ARQUITECTURA_OPENTTD.md).

---

## Cómo ejecutar (cuando compiles en local)

```bash
cargo run -p openttdrs-client
```

El cliente muestra una rejilla de depuración del mapa y el **tick** de simulación en el título de la ventana.

```bash
cargo test -p openttdrs-core
# o todo el workspace (incluye comprobaciones del mapa en openttdrs-core):
cargo test --workspace
```

Tras tocar `scripts/parse_sav.py`, conviene ejecutar `python3 scripts/verify_parse_sav_reference.py` (misma comprobación que en CI).

---

## Descarga de assets (gráficos, sonidos, música)

Para simplificar, usá el wrapper:

```bash
./scripts/descargar_assets.sh graficos --32bpp
./scripts/descargar_assets.sh sonidos
./scripts/descargar_assets.sh musica
```

También podés ejecutar todo junto:

```bash
./scripts/descargar_assets.sh todo --32bpp
```

Notas:
- El modo de gráficos es obligatorio: `--8bpp` o `--32bpp`.
- La cache de descargas se guarda en `/.downloads/openttd/` (ignorada por git).
- Los assets finales quedan bajo `assets/`.

Si preferís scripts individuales:

```bash
./scripts/descargar_graficos.sh --32bpp
./scripts/descargar_sonidos.sh --opensfx
./scripts/descargar_musica.sh --openmsx
```

---

## Savegames → `.ottdmap` (`scripts/parse_sav.py`)

Convierte un save de OpenTTD (`.sav`) al binario que carga el cliente (`MAP1`, cabecera versionada). **v5** extiende v4 con un byte por tesela de **MAP2** (bajo), **MAP7** y **M3HI** —en el motor OpenTTD el chunk `M3HI` es el byte **`m4()`** del mapa, no el “alto” de `m3`— y **v5+12** añade un byte por tesela del **alto de MAP2** cuando el save trae `MAP2` como `u16` (reserva PBS en bits altos). Footers opcionales: **INDP**, **STNN** (blob estaciones), **TNBP** (túnel/puente), **STXY** (lista explícita de teselas `MP_STATION` para el cliente sin decodificar `STNN`). Sigue aplicando la reconstrucción de **HouseID** en saves antiguos (`m8` desde M3HI/M3LO si la versión del save es &lt; 348).

Saves comprimidos con magic **OTTD** (LZO) requieren el paquete opcional `python-lzo` (`pip install python-lzo`); **OTTZ** (zlib) y **OTTX** (xz) no lo necesitan.

```bash
python3 scripts/parse_sav.py ruta/al/mapa.sav salida.ottdmap
OTTDMAP_FILE=salida.ottdmap cargo run -p openttdrs-client
```

Persistencia de la simulación (JSON del core) y atajos en el cliente:

```bash
OTTDJSON_LOAD=partida.json cargo run -p openttdrs-client   # arranque desde JSON
# En ventana: F5 guarda, F9 carga (por defecto openttdrs_sim.json; OPENTTDRS_JSON_SAVE para otra ruta).
# F9 redibuja suelo/vías/vehículos y mueve la cámara aunque el JSON cambie el tamaño del mapa.
```

Bases de sprites de señal (OpenGFX 8bpp por defecto): `OPENTTDRS_SIGNAL_BASE` y `OPENTTDRS_SIGNAL_ALT_BASE` (enteros 512–4096).

**Nota:** la carpeta `assets/` está en `.gitignore`. Los gráficos se generan con los scripts de la sección anterior; los `.ottdmap` que generes en local **no se versionan** salvo que los pongas en otro path (por ejemplo `tests/fixtures/` para pruebas).

Un “NewGRF completo” en el cliente no se reduce al `.ottdmap`: hacen falta los `.grf`, tablas de specs y lógica de dibujo; el export solo expone bytes de mapa y blobs de chunks (ver [docs/TILES_Y_SAVEGAMES_OPENTTD.md](docs/TILES_Y_SAVEGAMES_OPENTTD.md)).

**Regenerar el golden usado en CI** (tras cambiar la lógica de `parse_sav.py`):

```bash
python3 scripts/emit_parse_sav_golden.py tests/fixtures/stationlist-test.sav \
  > tests/fixtures/parse_sav_stationlist_golden.json
```

Comprobación manual respecto al golden:

```bash
python3 scripts/verify_parse_sav_reference.py
```

---

## Estructura del repo

```
Cargo.toml                 # Workspace
rust-toolchain.toml
crates/openttdrs-core/     # Mapa, tick, estado sin Bevy
crates/openttdrs-client/   # Binario Bevy
docs/                      # Informe de arquitectura upstream
scripts/                   # fetch-openttd-reference.sh, parse_sav, assets
tests/fixtures/            # .sav + golden JSON para verify_parse_sav (sí versionados)
.github/                   # CI + Dependabot
reference/                 # Clon local ignorado por git
```

---

## Licencia

El proyecto openttdrs se distribuye bajo **GPL-2.0** (ver archivo `LICENSE`). El código de OpenTTD usado como referencia mantiene su propia licencia y copyright; no asumas compatibilidad con otras licencias sin revisión explícita.
