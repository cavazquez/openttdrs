# openttdrs

<p align="center">
  <img src="static/app/openttdrs-icon.png" alt="openttdrs" width="220">
</p>

[![CI](https://github.com/cavazquez/openttdrs/actions/workflows/ci.yml/badge.svg)](https://github.com/cavazquez/openttdrs/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/cavazquez/openttdrs/graph/badge.svg)](https://codecov.io/gh/cavazquez/openttdrs)
[![Licencia GPL-2.0](https://img.shields.io/badge/licencia-GPL--2.0-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](https://doc.rust-lang.org/stable/releases.html)
[![Bevy](https://img.shields.io/badge/Bevy-0.19.0-C659D4.svg)](https://bevyengine.org/)
[![Inspiración OpenTTD](https://img.shields.io/badge/inspiración-OpenTTD-5a3.svg)](https://www.openttd.org/)

Simulador de transporte inspirado en [OpenTTD](https://www.openttd.org/), escrito en **Rust** con cliente [Bevy](https://bevyengine.org/). El desarrollo es **incremental**: siempre hay algo jugable; la paridad total (NewGRF completo, red, saves idénticos al original) se aborda por cortes documentados, no de golpe.

> Compilar Bevy puede saturar CPU/RAM. Si hace falta: `cargo build -j 1`, o dejá que [CI](.github/workflows/ci.yml) valide el build.

**Última actualización:** 2026-07-15

---

## Estado del proyecto

| Capa | Qué hay |
|------|---------|
| **Core** (`openttdrs-core`) | Mapa, tick, comandos, simulación road/rail, señales/PBS parcial, economía, saves JSON + import/export `.sav` / `.ottdmap` |
| **Cliente** (`openttdrs-client`) | Vista isométrica OpenGFX, menú de inicio, toolbar, listas UI, noticias, multi-compañía mínima |
| **NewGRF** | Action0–14 parse; Action1/2/3/5 con sprites in-world (trenes, stations, roadtypes, shore, catenary); vars de tesela/vehículo en runtime |
| **Hito 0.1** | Fundación I0–I7 hecha; solitario jugable. **I8 red** = [#21](https://github.com/cavazquez/openttdrs/issues/21) (post-0.1) |

**Trabajo reciente (jul 2026):** Action2 variational (trains/stations/road), procedure `7E` / `\2psto`, vars de vehículo y de tesela al dibujar. Issues de backlog: [issues abiertas](https://github.com/cavazquez/openttdrs/issues).

**Siguiente corte (roadmap):** red (#21, aplazada) u otro P3 — ver [ROADMAP_PARIDAD_UI_GLOBAL.md](docs/ROADMAP_PARIDAD_UI_GLOBAL.md). Editor #42 ✅ · GameScript-lite #43 ✅ (story/goals/league; Squirrel OOS).

---

## Arranque rápido

```bash
# 0) Diagnóstico de entorno (no adivinar qué falta)
./scripts/doctor.sh
# Si hay FAIL: ./scripts/doctor.sh --fix

# 1) Gráficos (obligatorio la primera vez; audio ya viene en git)
./scripts/descargar_assets.sh graficos --32bpp

# 2) Cliente (menú: Nueva partida / Cargar / Demo / Salir)
cargo run -p openttdrs-client
```

### Dependencias (máquina nueva)

`./scripts/doctor.sh` chequea toolchain Rust, paquetes APT (misma lista que CI en [`.github/apt-packages.txt`](.github/apt-packages.txt)), `grfcodec`, Python (`numpy` / `Pillow`), `pip` y assets. Con `--fix` imprime los comandos a correr.

```bash
# Libs Bevy (X11 / Wayland / ALSA / …) — misma lista que CI
sudo apt-get update
sudo apt-get install -y $(grep -v '^#' .github/apt-packages.txt | grep -v '^[[:space:]]*$')

# Decodificar OpenGFX + post-proceso de sprites
sudo apt-get install -y grfcodec python3-numpy python3-pil

# pip (opcional; alternativa a los paquetes APT de numpy/Pillow)
sudo apt-get install -y python3-pip
python3 -m pip install --user -r scripts/requirements-assets.txt
```

`descargar_graficos.sh` valida `numpy` y `Pillow` **antes** de borrar/descargar, para no fallar al final del pipeline.

| Asset | ¿En el repo? | Notas |
|-------|----------------|-------|
| Sonidos / música | Sí (`assets/sounds`, `assets/music`) | No hace falta regenerar para jugar |
| Gráficos OpenGFX | No | Una vez: script de arriba |
| Fuente UI | Sí (`static/fonts/`) | Fuera de `assets/` (ignorado por git) |

Otras formas de arrancar:

```bash
# Mapa desde fixture / save convertido
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/p6_p4_showcase.ottdmap cargo run -p openttdrs-client

# Partida JSON
OTTDJSON_LOAD=save/openttdrs_sim.json cargo run -p openttdrs-client

# Mundo procedural headless (sin menú)
OPENTTDRS_WORLD_GEN=1 OPENTTDRS_WORLD_ISLAND=1 OPENTTDRS_WORLD_SEED=42 cargo run -p openttdrs-client
```

En juego: **F5** guardar · **F9** cargar · pausa/velocidad en toolbar · preferencias en `~/.config/com.github.cavazquez.openttdrs/`.

---

## Desarrollo

```bash
./scripts/doctor.sh         # deps de sistema + toolchain + assets (antes de adivinar)
./scripts/check.sh          # fmt + clippy + tests (día a día)
./scripts/check.sh ci       # paridad con el job CI (TNBP, golden parse_sav, …)
./scripts/check.sh cov      # cobertura → lcov.info (cargo-llvm-cov)
cargo test --workspace
```

| Ruta | Responsabilidad |
|------|-----------------|
| `crates/openttdrs-core/` | Simulación, mapa, comandos, NewGRF parse, save/sav |
| `crates/openttdrs-client/` | Bevy, render, UI, bootstrap |
| `docs/` | Roadmaps e informes — índice: [docs/README.md](docs/README.md) |
| `scripts/` | Assets, `doctor.sh`, `check.sh`, `parse_sav.py`, referencia OpenTTD |
| `tests/fixtures/` | `.sav` + goldens versionados |

**Convención:** lógica de juego en core vía `Command` / `apply_command`; el cliente no mutea el mundo por su cuenta.

Referencia OpenTTD (clon local, no versionado):

```bash
./scripts/fetch-openttd-reference.sh   # → reference/openttd-upstream/
```

---

## CI y calidad

Un job en [.github/workflows/ci.yml](.github/workflows/ci.yml) (caché Cargo + APT):

| Paso | Contenido |
|------|-----------|
| `rustfmt` | `cargo fmt --all -- --check` |
| `clippy` | workspace, `-D warnings`, perfil `ci` |
| tests | PRs: `nextest --no-build`; push a `main`: `llvm-cov` → Codecov |
| extras | TNBP, golden `parse_sav`, `py_compile` |

Cobertura manual: [.github/workflows/coverage.yml](.github/workflows/coverage.yml) (`workflow_dispatch`) o `./scripts/check.sh cov`.

---

## Qué está hecho / qué falta (resumen)

Leyenda: ✅ hecho · 🟡 parcial · ❌ / 🔮 backlog (issues en GitHub)

| Área | Estado | Notas |
|------|--------|-------|
| Construcción road + rail + terraform | ✅ | Waypoints, señales, `RailConvert` (ciclo railtypes) |
| PBS / path signals | 🟡 | Reserva básica; afinado en issues |
| Economía + 6 cargos + packets | 🟡 | CargoDist MCF nivel 2 ✅; falta tabla temperate completa |
| Import `.sav` → mapa + flota | 🟡 | Roundtrip propio; OpenTTD oficial incompleto |
| Export `.sav` | 🟡 | Mapa+STNN+CITY+INDY+VEHS; órdenes avanzadas pendientes |
| Render OpenGFX vanilla | ✅ | Industrias gfx 0–174; NewGRF ≥175 MVP (#71) |
| UI solitario (menús, listas, noticias) | ✅ | UI-0…UI-7 cortes jugables |
| Multi-compañía | 🟡 | Mínima + ownership; segunda humana OOS |
| NewGRF Action0–14 + Action2 runtime | ✅ | Action1/3; estaciones `m5`+CB24; params `0x7F`; industrias ≥175 |
| Barcos / aviones | 🔮 | |
| Multijugador (I8) | 🔮 | [#21](https://github.com/cavazquez/openttdrs/issues/21) |
| IA rivales / GameScript / editor | 🟡 | IA + editor #42 ✅; GS-lite #43 ✅; Squirrel OOS |

Backlog vivo: [issues del repo](https://github.com/cavazquez/openttdrs/issues) (generadas desde los ROADMAP, jul 2026).

---

## Documentación

| Documento | Uso |
|-----------|-----|
| [docs/README.md](docs/README.md) | Índice de toda la carpeta `docs/` |
| [docs/ROADMAP_PARIDAD_UI_GLOBAL.md](docs/ROADMAP_PARIDAD_UI_GLOBAL.md) | Paridad UI + progreso NewGRF Action0–14 |
| [docs/ROADMAP_SPRINTS.md](docs/ROADMAP_SPRINTS.md) | Sprints hito 0.1 |
| [docs/PARIDAD_OPENTTD.md](docs/PARIDAD_OPENTTD.md) | Gaps vs OpenTTD |
| [docs/FLUJO_MAPA_Y_CLIENTE.md](docs/FLUJO_MAPA_Y_CLIENTE.md) | Save → `.ottdmap` → cliente → JSON |
| [docs/TILES_Y_SAVEGAMES_OPENTTD.md](docs/TILES_Y_SAVEGAMES_OPENTTD.md) | Bytes de mapa y saves |
| [docs/DISENO_INCREMENTAL.md](docs/DISENO_INCREMENTAL.md) | Filosofía I0–I8 |

Saves OpenTTD → mapa del cliente:

```bash
python3 scripts/parse_sav.py partida.sav salida.ottdmap
OTTDMAP_FILE=salida.ottdmap cargo run -p openttdrs-client
```

Detalle de planos/chunks: [docs/OTTDMAP_FORMAT.md](docs/OTTDMAP_FORMAT.md). Regenerar assets: `./scripts/descargar_assets.sh --help`.

---

## Stack

| Tecnología | Rol |
|------------|-----|
| Rust 2024 (MSRV **1.95**) | Workspace `openttdrs-core` + `openttdrs-client` |
| Bevy **0.19** | ECS, ventana, render 2D, UI |
| serde / JSON | Save/load del core |
| Python 3 + Pillow | `parse_sav`, goldens, recorte OpenGFX |
| OpenGFX / OpenSFX / OpenMSX | Arte, SFX y música |
| GitHub Actions + Dependabot | CI y deps mensuales |

---

## Estructura del repo

```
Cargo.toml                 # Workspace
crates/openttdrs-core/     # Simulación sin Bevy
crates/openttdrs-client/   # Binario Bevy
docs/                      # Roadmaps e informes
scripts/                   # check, assets, parse_sav, fetch upstream
tests/fixtures/            # .sav + goldens
.github/                   # CI + Dependabot
reference/                 # Clon OpenTTD (gitignored)
```

---

## Licencia

**GPL-2.0** (ver `LICENSE`). El código de OpenTTD usado como referencia conserva su propia licencia y copyright.
