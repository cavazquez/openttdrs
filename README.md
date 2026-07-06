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

Port **incremental** de ideas y mecánicas inspiradas en [OpenTTD](https://www.openttd.org/) hacia **Rust**, con motor gráfico [Bevy](https://bevyengine.org/). El objetivo a largo plazo es un simulador modular; la **paridad total** (NewGRF, red, saves idénticos) es un alcance opcional y costoso en tiempo.

> **Rendimiento en tu máquina:** compilar Bevy y dependencias puede ser pesado. Si notas saturación de CPU o RAM, usa por ejemplo `cargo build -j 1` o deja que el flujo de [CI](.github/workflows/ci.yml) valide el build en GitHub Actions.

**Roadmap:** hito [0.1 — vertical slice en solitario](https://github.com/cavazquez/openttdrs/milestone/1): fundación **I0–I7** ya en `main`; el foco actual son las fases **SP1–SP4** (ciclo jugable, construcción, visual, pulido). **I8 (red / multijugador)** queda en backlog de **mínima prioridad** hasta cerrar el juego de un jugador. Plan de sprints: [docs/ROADMAP_SPRINTS.md](docs/ROADMAP_SPRINTS.md); paridad vs OpenTTD: [docs/PARIDAD_OPENTTD.md](docs/PARIDAD_OPENTTD.md); spec: [docs/DISENO_INCREMENTAL.md](docs/DISENO_INCREMENTAL.md); índice de docs: [docs/README.md](docs/README.md).

**Reciente (jun 2026):** menú de inicio con **Nueva partida** (mapas 24×18…256², clima, año, semilla, densidad pueblos/industrias, capital inicial), fondo intro con isla procedural y tráfico decorativo, **población procedural** (pueblos con calles y casas, industrias) y **lagos/costas** en `world_gen`. Refactor de módulos grandes: `ui/main_menu/`, `bootstrap/procedural_population/`, `command/transport/`, `command/tests/`. Detalle menú: [docs/ROADMAP_MAIN_MENU.md](docs/ROADMAP_MAIN_MENU.md).

**Flujo de trabajo** (save → mapa → cliente → JSON): [docs/FLUJO_MAPA_Y_CLIENTE.md](docs/FLUJO_MAPA_Y_CLIENTE.md).

---

## Mapa de características

Tabla maestra para orientar desarrollo, correcciones y paridad con OpenTTD. **Actualizar esta sección** al cerrar una feature o cambiar su estado.

**Leyenda:** ✅ Hecho · 🟡 Parcial / simplificado · ❌ Pendiente · 🔮 Backlog largo plazo (post-0.1)

**Última actualización:** 2026-06-22

### Resumen por bloque

| Bloque | Estado |
|--------|--------|
| Fundación I0–I7 (core sin Bevy) | ✅ |
| Construcción carretera + ferrocarril | ✅ alto |
| Simulación, economía, 6 cargos | 🟡 |
| Import `.sav` → mapa + entidades | 🟡 |
| Render OpenGFX vanilla | 🟡 ~85–90 % |
| UI, toolbar, ventanas, barra inferior | 🟡 |
| Menú inicio + Nueva partida procedural | ✅ fase 2 |
| Señales bloque v1 (sin PBS) | 🟡 |
| Noticias / ticker / periódico | ✅ N1–N5 |
| Terraform / gen mundo / climas | 🟡 (terraform T1–T3; gen T4 MVP) |
| Barcos, aviones, Cargo Dist, NewGRF, red I8 | 🔮 |

### Guía rápida para continuar (IA / desarrolladores)

1. **Validar cambios:** `bash scripts/check.sh` (local) o `bash scripts/check.sh ci` (paridad CI).
2. **Crates:** lógica en `crates/openttdrs-core/`; Bevy/UI en `crates/openttdrs-client/`.
3. **Comandos de juego:** `crates/openttdrs-core/src/command/` → `apply.rs`, `preview.rs`, `transport/` (`road`, `rail`, `bridge`, `station`, `shared`), tests en `command/tests/`.
4. **Simulación:** `sim_step.rs`, `pathfinder.rs`, `rail_signals.rs`, `news.rs`.
5. **UI:** `crates/openttdrs-client/src/ui/` (toolbar, HUD, barra inferior en `ui/statusbar/`).
6. **Render:** `crates/openttdrs-client/src/render/`, sprites en `sprites/`.
7. **Import saves:** `scripts/parse_sav.py` (mapa) + `crates/openttdrs-core/src/sav/` (entidades).
8. **Roadmaps detallados:** ver columna *Referencia* abajo; índice en [docs/README.md](docs/README.md).
9. **OpenTTD upstream:** `./scripts/fetch-openttd-reference.sh` → `reference/openttd-upstream/`.

### Tabla completa

| Área | Característica | Estado | Referencia |
|------|----------------|--------|------------|
| **Fundación** | I0 — `GameTick`, calendario simulado | ✅ | `tick.rs`, `news.rs` (`format_calendar_date`) |
| **Fundación** | I1 — Mapa, teselas `m1`–`m8`, pendientes | ✅ | `map/`, `map/slope.rs` |
| **Fundación** | I2 — Industrias (producción 256 ticks) | ✅ | `industry.rs` |
| **Fundación** | I3 — Vehículos (bus, camión, tren) | ✅ | `vehicle.rs` |
| **Fundación** | I4 — Economía y 6 tipos de carga | 🟡 | `economy.rs`, `cargo.rs` — sin cargo packets |
| **Fundación** | I5 — Pathfinding A* carretera + vía direccional | ✅ | `pathfinder.rs` — no YAPF |
| **Fundación** | I6 — Comandos serializables `Command` | ✅ | `command/` |
| **Fundación** | I7 — Save/load JSON versionado (v4 + migraciones) | ✅ | `save.rs` |
| **Fundación** | I8 — Red / multijugador (replay comandos) | 🔮 | [DISENO_INCREMENTAL.md](docs/DISENO_INCREMENTAL.md) § I8 |
| **Mapa** | Formato binario `.ottdmap` (MAP1…v5+12) | ✅ | [OTTDMAP_FORMAT.md](docs/OTTDMAP_FORMAT.md) |
| **Mapa** | Carga mapa en cliente (`OTTDMAP_FILE`) | ✅ | `state/bootstrap/` |
| **Mapa** | Mapa demo procedural con layout jugable | ✅ | `demo_layout.rs` |
| **Mapa** | Nueva partida: tamaños 24×18…256², terreno procedural | ✅ | `state/bootstrap/world.rs`, `ui/main_menu/` |
| **Mapa** | Población procedural (pueblos + industrias) | 🟡 | `bootstrap/procedural_population/` — mapas ≥ 64² |
| **Mapa** | TNBP túneles/puentes JGR en import | ✅ | `map/tnbp.rs`, `TILES_Y_SAVEGAMES_OPENTTD.md` |
| **Mapa** | Alturas / pendientes (solo lectura + render) | ✅ | `map/slope.rs`, `iso/slope.rs` |
| **Import** | `parse_sav.py`: `.sav` → `.ottdmap` | ✅ | `scripts/parse_sav.py`, golden CI |
| **Import** | Parser Rust chunks estaciones/industrias | ✅ | `sav/` |
| **Import** | Entidades: vehículos, órdenes, dinero desde `.sav` | 🟡 | `sav/entities.rs`, `sav/orders.rs` — ver limitaciones en [TILES_Y_SAVEGAMES_OPENTTD.md](docs/TILES_Y_SAVEGAMES_OPENTTD.md) |
| **Import** | Partida OpenTTD 100 % jugable sin JSON propio | ❌ | Sprint 6 en [ROADMAP_SPRINTS.md](docs/ROADMAP_SPRINTS.md) |
| **Construcción** | Autorail carretera (drag, vecinos) | ✅ | `command/transport/road.rs` |
| **Construcción** | Depósito carretera | ✅ | `gen_road_depot_gfx_data.py`, `spawn_road_depot_tile`, preview multi-capa |
| **Construcción** | Paradas bus / camión (orientación RMB) | ✅ | `road_stop_gfx_data_generated.rs` |
| **Construcción** | Túnel / puente carretera | ✅ | `command/transport/bridge.rs`, `render/tiles/bridge.rs` |
| **Construcción** | Quitar carretera / limpiar tesela | ✅ | `Command::ClearTile`, `RemoveRoad*` |
| **Construcción** | Un solo sentido / drive-through carretera | 🟡 | Paridad parcial vs OpenTTD |
| **Construcción** | Autorail ferrocarril (curvas, cruce X\|Y) | ✅ | `command/transport/rail.rs` |
| **Construcción** | Depósito ferrocarril rotado | ✅ | `command/transport/rail.rs` |
| **Construcción** | Estación tren multi-tesela (1–7 × 1–7) | ✅ | `PlaceRailStationArea`, ventana picker |
| **Construcción** | Túnel / puente ferrocarril | ✅ | TNBP + comandos rail |
| **Construcción** | Quitar vía (`RailRemove`) | ✅ | `Command::RemoveRail` |
| **Construcción** | Waypoint ferroviario + orden pasar | ✅ | `PlaceRailWaypoint`, `StopKind::RailWaypoint` |
| **Construcción** | Colocar señal bloque (`PlaceRailSignal`) | ✅ | `rail_signals.rs`, toolbar rail |
| **Construcción** | Convertir tipo de vía (`RailConvert`) | ❌ | Botón stub en toolbar |
| **Construcción** | Terraform: elevar / bajar / nivelar | ✅ | [ROADMAP_TERRAFORM.md](docs/ROADMAP_TERRAFORM.md) T1–T3 |
| **Construcción** | Autoslope al construir sobre pendiente | ✅ | Terraform T3 |
| **Simulación** | Tick de juego, pausa, velocidad | ✅ | `simulation.rs`, `SimHudControls` |
| **Simulación** | Producción industria (10 specs sandbox) | ✅ | `industry.rs`, panel industria |
| **Simulación** | Carga / descarga en estaciones | ✅ | `sim_step.rs` |
| **Simulación** | Entrega carga → ingreso + popup `+$N` | ✅ | `economy.rs`, `income_popup.rs` |
| **Simulación** | Inflación y costes construcción | ✅ | `economy.rs` |
| **Simulación** | Cargo packets, rating estación, transit time | ❌ | Balances `u32` simplificados |
| **Simulación** | Servicio en depósito | ❌ | [PARIDAD_OPENTTD.md](docs/PARIDAD_OPENTTD.md) |
| **Simulación** | Señales: reserva bloque, un tren/bloque | 🟡 | `rail_signals.rs`, `sim_step.rs` — sin PBS |
| **Simulación** | PBS / path signals / presignals | ❌ | 🔮 Hito 0.2 |
| **Vehículos** | Compra / venta bus, camión, tren | ✅ | ventana compra, `command/` |
| **Vehículos** | Órdenes simples (ir a estación / tesela) | ✅ | `vehicle.rs`, panel órdenes |
| **Vehículos** | Flags `full_load` / `no_unload` | ✅ | `VehicleOrder`, import ORDL/ORDR |
| **Vehículos** | Órdenes condicionales / compartidas | ❌ | [PARIDAD_OPENTTD.md](docs/PARIDAD_OPENTTD.md) |
| **Vehículos** | Horarios (timetable) | ❌ | — |
| **Vehículos** | Barcos / aviones | 🔮 | Hito 0.3 |
| **Mundo** | Ciudades: demanda, etiquetas, ventana | ✅ | `town.rs`, `render/town_labels.rs` |
| **Mundo** | Pueblos generados con calles y casas terminadas | 🟡 | `procedural_population/towns.rs`, `Tile::completed_house` |
| **Mundo** | Panel industria (carga aceptada/producida) | ✅ | `ui/toolbar/industry_panel/` |
| **Mundo** | Subvencios / autoridad local | ❌ | — |
| **Mundo** | Generación procedural + 4 climas | 🟡 | `world_gen.rs` — colinas, bosques, **lagos**, modo isla |
| **Mundo** | Densidad pueblos/industrias y capital inicial | ✅ | `NewGameSettings`, menú Nueva partida |
| **Render** | Vista isométrica OpenGFX 8bpp/32bpp | ✅ | `iso/`, atlas PNG |
| **Render** | Terreno, agua, costa, rough | ✅ | `render/tiles/land.rs` |
| **Render** | Árboles multi-especie (1–4/tesela) | ✅ | [ROADMAP_PARIDAD_VISUAL.md](docs/ROADMAP_PARIDAD_VISUAL.md) §2 |
| **Render** | Campos / cercas (farmland) | ✅ | `gen_field_draw_data.py` |
| **Render** | Casas / edificios | ✅ | `sprites/house.rs` |
| **Render** | Industrias gfx 0–130 (tabla estática) | 🟡 | `industry_gfx_data_generated.rs` — [ROADMAP_INDUSTRIAS_PARIDAD.md](docs/ROADMAP_INDUSTRIAS_PARIDAD.md) |
| **Render** | Industrias gfx 131–174 + anim `draw_proc` | ❌ | Roadmap industrias P8+ |
| **Render** | Fundaciones industria en pendiente | 🟡 | Parcial |
| **Render** | Estaciones rail / road / waypoint | ✅ | `sprites/station.rs` |
| **Render** | Puente tablero sobre agua (road/rail) | ✅ | `render/tiles/bridge.rs` |
| **Render** | Junctions vía en pendiente | 🟡 | [ROADMAP_SPRINTS.md](docs/ROADMAP_SPRINTS.md) S3 |
| **Render** | Culling viewport mapas grandes | 🟡 | `render/world.rs` — solo mapas grandes |
| **Render** | Paleta compañía (Remap) | ✅ | `company_colour` |
| **UI** | Menú principal (Nueva partida, Cargar, Demo, Salir) | ✅ | `ui/main_menu/`, [ROADMAP_MAIN_MENU.md](docs/ROADMAP_MAIN_MENU.md) |
| **UI** | Fondo intro: isla procedural, paneo, agua animada, tráfico | ✅ | `ui/main_menu_intro.rs` |
| **UI** | Toolbar superior (construcción, sim) | ✅ | `ui/toolbar/` |
| **UI** | Preview fantasma (ghost) | ✅ | `ui/toolbar/preview/` |
| **UI** | Minimap | ✅ | `ui/toolbar/minimap/` |
| **UI** | Ventanas: vehículo, órdenes, estación, industria, pueblo, compra | ✅ | `ui/toolbar/*_panel/`, `*_window/` |
| **UI** | HUD alertas («sin ruta», «sin carga», incompatible) | ✅ | `ui/hud/display/` — [SP1_CHECKLIST.md](docs/SP1_CHECKLIST.md) |
| **UI** | Barra inferior: fecha \| centro \| dinero | ✅ | `ui/statusbar/` — N1 |
| **UI** | Ticker noticias (scroll headline) | ✅ | `ui/statusbar/sync.rs` — N2 |
| **UI** | Cartel noticias (sube desde abajo) | ✅ | N3 — entrega carga, primera entrega, primer vehículo |
| **UI** | Ticker avisos vehículo (sin ruta, sin carga, …) | ✅ | `news.rs` — `VehicleAdvice` + `poll_vehicle_advice_news` |
| **UI** | Clic noticia / ticker → centrar cámara | ✅ | `camera.rs`, `statusbar/sync.rs` |
| **UI** | Historial noticias (lista scroll) | ✅ | Clic en fecha barra → `statusbar/history.rs` |
| **UI** | Purga noticias antiguas (~2 años) | ✅ | `news.rs` — `maybe_purge_old_news` |
| **UI** | Config noticias Off/Summary/Full por tipo | ✅ | Ajustes → «Noticias…», prefs TOML |
| **UI** | Clic dinero → ventana finanzas | ✅ | `finances_window.rs` — efectivo, préstamo, stats |
| **UI** | Guardar F5 / cargar F9 JSON | ✅ | `state/json_persist.rs` |
| **UI** | Preferencias cliente (`~/.config/...`) | ✅ | audio, minimapa, ruta save |
| **Audio** | SFX construcción OK / error / ingreso | ✅ | `preparar_sonidos_hud.sh`, `sound_ping.rs` |
| **Audio** | SFX noticias (ticker, aplauso, chime) | ✅ | `news_*.wav` vía `preparar_sonidos_hud.sh` (OpenSFX) |
| **Audio** | Música ambiente in-game | ❌ | Sprint 5 en [ROADMAP_SPRINTS.md](docs/ROADMAP_SPRINTS.md) |
| **Calidad** | `check.sh` + CI (fmt, clippy, tests, TNBP, golden) | ✅ | `.github/workflows/ci.yml` |
| **Calidad** | Tests integración ciclo jugable SP1 | ✅ | `tests/sp1_playable_cycle.rs` |
| **Calidad** | Codecov / llvm-cov | ✅ | badge README |
| **Backlog** | Cargo Dist (link graph) | 🔮 | [PARIDAD_OPENTTD.md](docs/PARIDAD_OPENTTD.md) |
| **Backlog** | NewGRF runtime (gfx ≥ 175, props, callbacks) | 🔮 | [ROADMAP_INDUSTRIAS_PARIDAD.md](docs/ROADMAP_INDUSTRIAS_PARIDAD.md) §D |
| **Backlog** | Multijugador I8 | 🔮 | [DISENO_INCREMENTAL.md](docs/DISENO_INCREMENTAL.md) |

**Fases SP (hito 0.1):**

| Fase | Estado | Doc |
|------|--------|-----|
| SP2 Construcción | ✅ Cerrado 2026-05-22 | [SP2_CHECKLIST.md](docs/SP2_CHECKLIST.md) |
| SP4 Pulido / saves | ✅ Cerrado 2026-06-22 | [ROADMAP_SPRINTS.md](docs/ROADMAP_SPRINTS.md) S1 |
| SP3 Visual | 🟡 ~90 % | [ROADMAP_PARIDAD_VISUAL.md](docs/ROADMAP_PARIDAD_VISUAL.md) |
| SP1 Ciclo jugable | 🟡 Auto ✅ / manual pendiente | [SP1_CHECKLIST.md](docs/SP1_CHECKLIST.md) |

**Sprints operativos pendientes:** S2 resto (`RailConvert`), S3 visual, S4 SP1 manual, S5 señales+audio, S6 import — ver [ROADMAP_SPRINTS.md](docs/ROADMAP_SPRINTS.md).

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
| ✅ | [GitHub Actions](https://docs.github.com/en/actions) | Ver [CI y calidad](#ci-y-calidad). Caché: **rust-cache** (Cargo/registry/`target`) + **APT** (libs del sistema). |
| 🤖 | [Dependabot](https://docs.github.com/en/code-security/dependabot) | Actualizaciones **mensuales** de Cargo y Actions (`.github/dependabot.yml`). |
| 📚 | OpenTTD upstream | Solo referencia local; ver sección [Código de referencia](#código-de-referencia-openttd-no-versionado). |

**MSRV:** el workspace declara `rust-version = 1.95.0` (estable actual); [Bevy 0.19.0](https://crates.io/crates/bevy) exige como mínimo 1.95.0. `rust-toolchain.toml` fija **1.95.0** con `rustfmt` y `clippy`.

---

## CI y calidad

El workflow [.github/workflows/ci.yml](.github/workflows/ci.yml) en cada push/PR a `main` ejecuta (con caché para no recompilar ni reinstalar todo desde cero):

| Caché | Qué guarda |
|-------|------------|
| [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) | `~/.cargo/registry`, `target/` (clave compartida `openttdrs`) |
| [gerlero/apt-install](https://github.com/gerlero/apt-install) | Paquetes APT (X11, Vulkan, ALSA) vía [.github/composite/linux-build-deps](.github/composite/linux-build-deps) |

**Rendimiento:** un solo job `check`. `clippy --all-targets` compila el workspace **una vez**; en PRs `nextest run --no-build` ejecuta tests sin recompilar. En push a `main`, `llvm-cov nextest` sustituye a nextest (2ª compilación instrumentada para LCOV, pero sin un segundo job). Tiempo típico: PR ~8–12 min, push a `main` ~12–16 min (antes ~20 min con dos jobs).

Pasos:

| Paso | Qué valida |
|------|----------------|
| 🎨 `rustfmt` | `cargo fmt --all -- --check` |
| 📎 `clippy` | `cargo clippy --workspace --all-targets --profile ci -- -D warnings` |
| 🧪 `nextest` | PR: `cargo nextest run --profile ci --no-build` (perfil en `.config/nextest.toml`) |
| 📊 `llvm-cov` | Push `main`: `cargo llvm-cov nextest --profile ci` → `lcov.info` + Codecov |
| 🗺️ TNBP | `cargo run -p openttdrs-core --example validate_ottdmap_tnbp` |
| 🐍 Golden / Python | scripts de mapas y `parse_sav` |

Cobertura manual sin CI completo: [.github/workflows/coverage.yml](.github/workflows/coverage.yml) (`workflow_dispatch`).

En local, paridad con el job **check** (sin Codecov):

```bash
./scripts/check.sh ci
```

| Modo | Uso |
|------|-----|
| `./scripts/check.sh` | Desarrollo: `fmt` + clippy + tests |
| `./scripts/check.sh ci` | Igual que CI: `fmt-check`, clippy, tests, TNBP, golden `parse_sav`, `py_compile` |
| `./scripts/check.sh fmt-check` | Solo formato (como el job `lint`) |
| `./scripts/check.sh lint` | Solo clippy `-D warnings` |
| `./scripts/check.sh test` | Solo tests del workspace |
| `./scripts/check.sh cov` | Tests + `lcov.info` (requiere `cargo-llvm-cov`) |
| `./scripts/check.sh build` | `cargo build --workspace` (cliente Bevy) |

Regresión construcción (checklist SP2): `cargo test -p openttdrs-core --lib command` y `cargo test -p openttdrs-client preview::`.

---

## Cobertura de tests

En **CI**, la cobertura corre solo en **push a `main`** (mismo job `check`, tras clippy): [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) con `cargo llvm-cov nextest` genera **`lcov.info`** y lo sube a [Codecov](https://codecov.io/gh/cavazquez/openttdrs). En **PRs** no se repite esa compilación instrumentada: solo clippy + nextest `--profile ci --no-build`.

Relanzar cobertura a mano: workflow [.github/workflows/coverage.yml](.github/workflows/coverage.yml) (`workflow_dispatch`).

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
# Cliente con menú de inicio (Nueva partida por defecto: 64², isla + lagos)
cargo run -p openttdrs-client

# Mapa desde fixture o save convertido (salta menú → juego directo)
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/p6_p4_showcase.ottdmap cargo run -p openttdrs-client

# Partida guardada JSON
OTTDJSON_LOAD=save/openttdrs_sim.json cargo run -p openttdrs-client

# Headless / CI: opciones vía env (sin menú)
OPENTTDRS_WORLD_GEN=1 OPENTTDRS_WORLD_ISLAND=1 OPENTTDRS_WORLD_SEED=42 cargo run -p openttdrs-client
```

Al arrancar sin `OTTDMAP_FILE` ni `OTTDJSON_LOAD`, el **menú principal** ofrece **Nueva partida** (clima, tamaño, terreno procedural, densidad, capital), **Cargar partida**, **Demo clásica** (24×18 tutorial) y **Salir**. En juego: toolbar, barra inferior (fecha / compañía / dinero), HUD de alertas. Atajos: **F5** guardar, **F9** cargar JSON, pausa/velocidad en toolbar. En Nueva partida: **1–4** clima, **Z/X** densidad, **Enter** iniciar.

```bash
cargo test -p openttdrs-core
# o todo el workspace (incluye comprobaciones del mapa en openttdrs-core):
cargo test --workspace
```

Tras tocar `scripts/parse_sav.py`, conviene ejecutar `python3 scripts/verify_parse_sav_reference.py` (misma comprobación que en CI).

---

## Jugar tras `git clone` (sin `apt` extra)

El cliente necesita **Rust** y los assets que el repo ya versiona o que generás una vez:

| Asset | ¿En el repo? | ¿Necesitás `apt`/scripts? |
|-------|----------------|---------------------------|
| **Sonidos** (`assets/sounds/*.wav`) | Sí (~22 WAV OpenSFX) | No para jugar |
| **Música** (`assets/music/*.ogg`) | Sí (theme + 27 pistas OpenMSX) | No para jugar |
| **Gráficos** (`assets/opengfx/`) | No (muy grande) | Sí, una vez: ver abajo |
| **Fuente UI** (`static/fonts/`) | Sí | No |

```bash
# Primera vez (solo gráficos; audio ya viene en git)
./scripts/descargar_assets.sh graficos --32bpp

cargo run -p openttdrs-client
```

No hace falta instalar `fluidsynth`, `catcodec` ni `ffmpeg` **solo para jugar**: esas herramientas sirven para **regenerar** audio o gráficos (mantenedores).

### Dependencias opcionales (solo regenerar assets)

| Herramienta | Para qué | Ubuntu/Debian |
|-------------|----------|----------------|
| **grfcodec** | Sprites OpenGFX desde `.grf` | `sudo apt install grfcodec` |
| **catcodec** | WAV desde `opensfx.cat` | compilar desde [OpenTTD/catcodec](https://github.com/OpenTTD/catcodec) o usar `.downloads/catcodec/build/` |
| **fluidsynth** + **fluid-soundfont-gm** | MIDI OpenMSX → WAV | `sudo apt install fluidsynth fluid-soundfont-gm` |
| **ffmpeg** | WAV → OGG | `sudo apt install ffmpeg` |

Scripts de regeneración:

```bash
./scripts/preparar_sonidos_opensfx.sh   # sonidos mundo + HUD
./scripts/preparar_musica_ogg.sh        # playlist OpenMSX (28 OGG)
./scripts/descargar_assets.sh graficos --32bpp
```

---

## Descarga de assets (gráficos, sonidos, música)

Para simplificar, usá el wrapper (útil si querés **regenerar** o actualizar basesets; el audio ya está en git):

```bash
./scripts/descargar_assets.sh graficos --32bpp   # obligatorio la primera vez
./scripts/descargar_assets.sh sonidos            # opcional si ya tenés assets/sounds/*.wav
./scripts/descargar_assets.sh musica             # opcional si ya tenés assets/music/*.ogg
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
python3 scripts/extract_train_vehicle_sprites.py   # locomotoras (5 grupos)
python3 scripts/gen_vehicle_gfx_data.py
./scripts/descargar_sonidos.sh --opensfx   # OpenSFX + WAV (o usar preparar_sonidos_opensfx.sh)
./scripts/descargar_musica.sh --openmsx     # OpenMSX + OGG (o usar preparar_musica_ogg.sh)
```

Solo copiar sonidos/música ya descargados: `./scripts/preparar_sonidos_opensfx.sh` y `./scripts/preparar_musica_ogg.sh`.

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
# Preferencias del cliente (ruta de guardado, minimapa, audio, debug): `~/.config/com.github.cavazquez.openttdrs/` (Linux).
# `OPENTTDRS_DEBUG=1` overlay FPS; `OPENTTDRS_GIZMOS=1` etiquetas de debug en el mapa.
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
