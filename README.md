# openttdrs

<p align="center">
  <img src="static/app/openttdrs-icon.png" alt="openttdrs" width="220">
</p>

[![CI](https://github.com/cavazquez/openttdrs/actions/workflows/ci.yml/badge.svg)](https://github.com/cavazquez/openttdrs/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/cavazquez/openttdrs/graph/badge.svg)](https://codecov.io/gh/cavazquez/openttdrs)
[![Licencia GPL-2.0](https://img.shields.io/badge/licencia-GPL--2.0-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://doc.rust-lang.org/stable/releases.html)
[![Bevy](https://img.shields.io/badge/Bevy-0.18.1-C659D4.svg)](https://bevyengine.org/)
[![Inspiración OpenTTD](https://img.shields.io/badge/inspiración-OpenTTD-5a3.svg)](https://www.openttd.org/)

Port **incremental** de ideas y mecánicas inspiradas en [OpenTTD](https://www.openttd.org/) hacia **Rust**, con motor gráfico [Bevy](https://bevyengine.org/). El objetivo a largo plazo es un simulador modular; la **paridad total** (NewGRF, red, saves idénticos) es un alcance opcional y costoso en tiempo.

> **Rendimiento en tu máquina:** compilar Bevy y dependencias puede ser pesado. Si notas saturación de CPU o RAM, usa por ejemplo `cargo build -j 1` o deja que el flujo de [CI](.github/workflows/ci.yml) valide el build en GitHub Actions.

**Roadmap:** hito [0.1 — vertical slice](https://github.com/cavazquez/openttdrs/milestone/1) con **8 incrementos** (I1–I8, issues [#14](https://github.com/cavazquez/openttdrs/issues/14)–[#21](https://github.com/cavazquez/openttdrs/issues/21)). Spec detallada en [docs/DISENO_INCREMENTAL.md](docs/DISENO_INCREMENTAL.md); correspondencia con el código C++ del upstream en [docs/INFORME_ARQUITECTURA_OPENTTD.md](docs/INFORME_ARQUITECTURA_OPENTTD.md).

**Flujo de trabajo** (save → mapa → cliente → JSON): [docs/FLUJO_MAPA_Y_CLIENTE.md](docs/FLUJO_MAPA_Y_CLIENTE.md).

---

## Stack tecnológico

| Icono | Tecnología | Descripción |
|-------|------------|-------------|
| 🦀 | [Rust](https://www.rust-lang.org/) | Lenguaje y toolchain; edición **2024** en el workspace. |
| 📦 | [Cargo](https://doc.rust-lang.org/cargo/) | Workspace con crates `openttdrs-core` y `openttdrs-client`. |
| 🎮 | [Bevy](https://bevyengine.org/) | ECS, ventana, cámara 2D / isométrica, UI, estados (cliente). |
| 🖼️ | [wgpu](https://wgpu.rs/) (vía Bevy) | API gráfica bajo el render 2D; en CI se usa **Vulkan** por software (`mesa-vulkan-drivers`). |
| 🪟 | [winit](https://github.com/rust-windowing/winit) | Ventanas y entrada (vía Bevy); el cliente se compila con **X11** (sin Wayland por defecto). |
| 🧩 | [serde](https://serde.rs/) + [serde_json](https://docs.rs/serde_json) | Persistencia y carga de estado en `openttdrs-core`. |
| 🐍 | [Python](https://www.python.org/) 3 | `parse_sav.py`, golden tests, generación de fixtures `.ottdmap`. |
| 🖌️ | [Pillow](https://python-pillow.org/) (`PIL`) | Recorte de sprites OpenGFX/OpenGFX2 en `descargar_graficos.sh`. |
| 🗺️ | Formato **`.ottdmap`** | Binario versionado (`MAP1`…); spec en [docs/OTTDMAP_FORMAT.md](docs/OTTDMAP_FORMAT.md) y [docs/TILES_Y_SAVEGAMES_OPENTTD.md](docs/TILES_Y_SAVEGAMES_OPENTTD.md). |
| 🎨 | [OpenGFX](https://github.com/OpenTTD/OpenGFX) / OpenGFX2 | Assets 8bpp / 32bpp extraídos con **grfcodec** + scripts del repo. |
| 🧪 | Tests + Clippy | `cargo test --workspace`; Clippy con **`-D warnings`** en CI. |
| ✅ | [GitHub Actions](https://docs.github.com/en/actions) | Ver [CI y calidad](#ci-y-calidad). |
| 🤖 | [Dependabot](https://docs.github.com/en/code-security/dependabot) | Actualizaciones **mensuales** de Cargo y Actions (`.github/dependabot.yml`). |
| 📚 | OpenTTD upstream | Solo referencia local; ver sección [Código de referencia](#código-de-referencia-openttd-no-versionado). |

**MSRV:** el workspace declara `rust-version` alineado con [Bevy 0.18.1](https://crates.io/crates/bevy) (consulta el `Cargo.toml` raíz). `rust-toolchain.toml` usa el canal `stable` con `rustfmt` y `clippy`.

---

## CI y calidad

El workflow [.github/workflows/ci.yml](.github/workflows/ci.yml) en cada push/PR a `main` ejecuta:

| Paso | Qué valida |
|------|----------------|
| 🎨 `rustfmt` | `cargo fmt --all -- --check` |
| 📎 `clippy` | `cargo clippy --workspace --all-targets -- -D warnings` |
| 📊 `llvm-cov` | `cargo llvm-cov --workspace --all-targets --lcov` (misma corrida que los tests; genera `lcov.info`) |
| ☁️ Codecov | Sube `lcov.info` (repo público; opcional `CODECOV_TOKEN` en *secrets* si Codecov lo pide) |
| 📦 Artefacto | `coverage-lcov` con `lcov.info` descargable desde la ejecución del workflow |
| 🗺️ TNBP | `cargo run -p openttdrs-core --example validate_ottdmap_tnbp` sobre fixture `v5p12_tnbp.ottdmap` |
| 🐍 Golden `parse_sav` | `python3 scripts/verify_parse_sav_reference.py` |
| ✔️ Python | `py_compile` de los scripts usados en el flujo de mapas |
| 🔨 `build` | `cargo build --workspace` (incluye cliente Bevy) |

En local, atajo equivalente (sin TNBP explícito salvo que lo añadas): `./scripts/check.sh ci`.

---

## Cobertura de tests

En **CI** cada push/PR ejecuta [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) en lugar de un `cargo test` duplicado: compila con instrumentación, corre **todos** los tests del workspace y produce **`lcov.info`**. Ese archivo se sube a [Codecov](https://codecov.io/gh/cavazquez/openttdrs) y también queda como **artefacto** `coverage-lcov` en GitHub Actions.

**Primera vez en Codecov:** entrá con GitHub a Codecov, activá el repo `cavazquez/openttdrs` si no aparece solo; el badge del README puede mostrar *unknown* hasta el primer informe exitoso.

**En local** (una vez: `rustup component add llvm-tools-preview` y `cargo install cargo-llvm-cov`):

```bash
./scripts/check.sh cov
# o HTML interactivo:
cargo llvm-cov --workspace --all-targets --html --open
```

Alternativa clásica: [cargo-tarpaulin](https://github.com/xd0092/tarpaulin).

**Siguientes refinamientos (opcionales):** umbral mínimo de cobertura en CI (`codecov.yml` o `cargo llvm-cov --fail-under-lines 50`), o ignorar crates generados en el informe.

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
- Fuente versionada: `static/fonts/DejaVuSansMono.ttf` se mantiene fuera de `assets/` para que la
  UI de Bevy pueda mostrar texto UTF-8 con acentos (`Fábrica`, `Refinería`, etc.). `assets/`
  sigue siendo generado/descargado y queda ignorado.
- Icono versionado de la aplicación: `static/app/openttdrs-icon.png`. El cliente lo carga al crear
  la ventana para mostrarlo en la barra de título y en el dock/barra de Ubuntu; si lo cambiás,
  cerrá y volvé a abrir la app.

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
# En ventana: F5 guarda, F9 carga (por defecto save/openttdrs_sim.json; OPENTTDRS_JSON_SAVE para otra ruta).
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
