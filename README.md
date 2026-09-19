# openttdrs

<p align="center">
  <img src="static/app/openttdrs-icon.png" alt="openttdrs" width="220">
</p>

[![CI](https://github.com/cavazquez/openttdrs/actions/workflows/ci.yml/badge.svg)](https://github.com/cavazquez/openttdrs/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/cavazquez/openttdrs/graph/badge.svg)](https://codecov.io/gh/cavazquez/openttdrs)
[![Licencia GPL-2.0-only](https://img.shields.io/badge/licencia-GPL--2.0--only-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.98%2B-orange.svg)](https://doc.rust-lang.org/stable/releases.html)
[![Bevy](https://img.shields.io/badge/Bevy-0.19.0-C659D4.svg)](https://bevyengine.org/)
[![Snap Store](https://snapcraft.io/openttdrs/badge.svg)](https://snapcraft.io/openttdrs)
[![Inspiración OpenTTD](https://img.shields.io/badge/inspiración-OpenTTD-5a3.svg)](https://www.openttd.org/)

Simulador de transporte inspirado en [OpenTTD](https://www.openttd.org/), escrito en **Rust** con cliente [Bevy](https://bevyengine.org/). El desarrollo es **incremental**: siempre hay algo jugable; la paridad total (NewGRF completo, red, saves idénticos al original) se aborda por cortes documentados, no de golpe.

La alpha pública actual es [`0.1.0-alpha.1`](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1): hay paquetes de escritorio en GitHub y un [Snap para Linux amd64](https://snapcraft.io/openttdrs) en `latest/edge`. Es un canal de pruebas, no una promesa de estabilidad ni de paridad con OpenTTD.

> Compilar Bevy puede saturar CPU/RAM. Si hace falta: `cargo build -j 1`, o dejá que [CI](.github/workflows/ci.yml) valide el build. Las ejecuciones repetidas de `./scripts/check.sh` aprovechan `sccache` automáticamente cuando está instalado.

**Gobierno:** [CONTRIBUTING.md](CONTRIBUTING.md) · [SECURITY.md](SECURITY.md) · [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) · [ADRs](docs/adr/)

**Última actualización:** 2026-09-19

---

## Estado del proyecto

> Dirección actualizada: 2026-09-19. Las capacidades técnicas conservan sus
> matrices de evidencia; completar este corte no certifica paridad global.

**Corte completado: Primera ruta.** Desde el menú se puede abrir el escenario,
construir un servicio de camiones mina → central eléctrica, cobrar, guardar y
reanudar en Temperate 64×64 con OpenGFX y ES/EN. #572–#576 entregaron la ruta
por comandos, continuación JSON, entrada de menú, guía y feedback visible de
F5/F9; [#577](https://github.com/cavazquez/openttdrs/issues/577) certificó el
arranque gráfico del paquete Linux fuera del checkout y la sesión completa de
aceptación. El [plan ejecutable](docs/parity/continuous-work-plan.md) registra
el cierre: no queda un issue activo en este corte. La
[auditoría](docs/audits/2026-09-18-direction.md) explica la selección original.
La distribución de ese corte está publicada como
[prerelease de GitHub](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
y [Snap Store `latest/edge`](https://snapcraft.io/openttdrs).

| Capa | Qué hay |
|------|---------|
| **Core** (`openttdrs-core`) | Mapa, tick, comandos, simulación road/rail, señales/PBS parcial, economía, saves JSON + import/export `.sav` / `.ottdmap`; alcance `.sav` en la [matriz canónica](docs/parity/sav-compatibility.md) |
| **Cliente** (`openttdrs-client`) | Vista isométrica OpenGFX, menú de inicio, toolbar, listas UI, noticias; `--server` / `--client` (I8) |
| **Red** (`openttdrs-net`) | TCP lockstep + bin `openttdrs-dedicated` ([ADR 0001](docs/adr/0001-multiplayer-v1.md)) |
| **NewGRF** | Catálogos Action0/3/5 y runtime parcial; las matrices de [propiedades](docs/parity/newgrf-action0-matrix.md) y [callbacks](docs/parity/newgrf-callback-matrix.md) distinguen parseado, almacenado y ejecutado |
| **Hito 0.1** | [`0.1.0-alpha.1` publicada](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1); solitario jugable y [Snap `latest/edge`](https://snapcraft.io/openttdrs) para Linux amd64. **I8 red** MVP ([#21](https://github.com/cavazquez/openttdrs/issues/21) ✅) + host migration ([#171](https://github.com/cavazquez/openttdrs/issues/171), [ADR 0004](docs/adr/0004-host-migration-post-v1.md)) |

**Antecedentes de worldgen (cortes hasta septiembre 2026):** se alinearon las fases del generador
procedural (`landscape` → `clear` → `towns` → `industries` → `objects` →
`trees`) con OpenTTD para las cohortes canónicas, incluyendo los bucles de
teselas, costas, industrias, árboles y bocas de puentes/túneles. Las semillas
Toyland 512² `1330935378`–`1330935381` coinciden en las seis fases auditadas,
incluido el despeje completo de casas multitile al crear Toy Shops; la
semilla Arctic 512² `1330935382` también coincide en las seis fronteras tras
alinear `TileLoopTreesAlps`, el escalado de faros con bordes fluviales y el
rechazo de `MP_VOID`/preservación de `RoughSnow` en `MAP2`; la cohorte Arctic
512² `1330935378`–`1330935381` también queda exacta tras validar las cabezas de
puente con `CheckBridgeSlope`; la generalización a otras semillas, tamaños,
climas y configuraciones sigue abierta. Los detalles y
alcances pendientes viven en el [plan continuo de
paridad](docs/parity/continuous-work-plan.md) y sus matrices; al 2026-09-19 no
hay [issues de producto abiertos](https://github.com/cavazquez/openttdrs/issues).

**Arranque desde checkout (septiembre 2026):** el atlas OpenGFX 8bpp, la fuente,
los sonidos y la música están versionados. `cargo run` selecciona el cliente y,
en el primer inicio, materializa localmente los PNG que necesita la UI desde el
atlas incluido. No descarga assets ni requiere ejecutar scripts auxiliares.

**Próxima dirección:** no hay una tarea activa. El siguiente corte requiere una
nueva auditoría y un issue atómico, sin reactivar por defecto brechas de
raster/SAV/NewGRF o expansión multiclima. Sus límites técnicos siguen
documentados. Editor #42 ✅ · GameScript-lite #43 ✅ · IA TransCargo ✅
(Squirrel OOS).

---

## Arranque rápido

> Actualizado: 2026-09-19.

### Instalar la alpha publicada

En Linux **amd64**, la vía más corta es el [Snap Store](https://snapcraft.io/openttdrs):

~~~bash
sudo snap install openttdrs --channel=latest/edge
openttdrs
~~~

`latest/edge` es el canal alpha del proyecto. El Snap es estricto, incluye los
assets libres y guarda sus partidas JSON por defecto en
`~/snap/openttdrs/common/save/`; una actualización de revisión no borra esa
carpeta. El servidor dedicado queda disponible como `openttdrs.dedicated`.

También hay paquetes para Linux x86_64, Windows x86_64 y macOS arm64 en la
[prerelease `v0.1.0-alpha.1`](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1).
Descargá el archivo de tu plataforma y verificá el `.sha256` asociado antes de
extraerlo completo.

### Ejecutar desde el checkout

```bash
git clone https://github.com/cavazquez/openttdrs.git
cd openttdrs
cargo run
```

Eso abre el menú con **Nueva partida / Primera ruta / Cargar / Demo / Salir**. En el primer
inicio se crean bajo `assets/opengfx/tiles/` los PNG derivados del atlas
versionado; no se usa red, `grfcodec`, Python ni un script de preparación. La
carpeta derivada está ignorada por Git y se reconstruye automáticamente si falta.

### Dependencias (máquina nueva)

> Actualizado: 2026-09-19.

Para jugar desde el checkout hacen falta Rust **1.98+** y las bibliotecas nativas
de ventana/audio que use tu distribución. En Ubuntu/Debian, la misma lista que
CI está en [`.github/apt-packages.txt`](.github/apt-packages.txt). `cargo` baja
sus dependencias Rust normalmente; no hay un paso manual de assets.

```bash
# Libs Bevy (X11 / Wayland / ALSA / …) — misma lista que CI
sudo apt-get update
sudo apt-get install -y $(grep -v '^#' .github/apt-packages.txt | grep -v '^[[:space:]]*$')

# Sólo para contribuir o regenerar/cambiar el baseset gráfico, no para jugar:
sudo apt-get install -y grfcodec python3-numpy python3-pil
```

`./scripts/doctor.sh` queda como diagnóstico para desarrollo y
`./scripts/descargar_assets.sh graficos --32bpp` sólo sirve para regenerar o
probar un baseset alternativo; el flujo normal de jugador no los necesita.

| Asset | ¿En el repo? | Notas |
|-------|----------------|-------|
| Sonidos / música | Sí (`assets/sounds`, `assets/music`) | Listos tras `git clone` |
| Gráficos OpenGFX | Sí (`assets/opengfx/atlas/`) | `cargo run` deriva `tiles/` localmente una vez |
| Fuente UI | Sí (`static/fonts/`) | Lista tras `git clone` |

Otras formas de arrancar:

```bash
# Mapa desde fixture / save convertido
OTTDMAP_FILE=crates/openttdrs-core/tests/fixtures/p6_p4_showcase.ottdmap cargo run

# Partida JSON
OTTDJSON_LOAD=save/openttdrs_sim.json cargo run

# Mundo procedural headless (sin menú)
OPENTTDRS_WORLD_GEN=1 OPENTTDRS_WORLD_ISLAND=1 OPENTTDRS_WORLD_SEED=42 cargo run
```

En juego: **F5** guarda · **F9** carga · pausa/velocidad están en la toolbar.
Desde el checkout, las preferencias viven en
`~/.config/com.github.cavazquez.openttdrs/`; desde el Snap, la partida JSON
predeterminada vive en `~/snap/openttdrs/common/save/`.

---

## Desarrollo

> Actualizado: 2026-09-19.

Flujo de PRs y DoD: [CONTRIBUTING.md](CONTRIBUTING.md). Capas: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

```bash
./scripts/doctor.sh         # diagnóstico de toolchain y pipeline opcional de assets
./scripts/check.sh          # fmt + clippy + tests (día a día)
./scripts/check.sh ci       # núcleo compartido con ci.yml (ver excepciones GHA en check.sh)
./scripts/check.sh ci-python  # solo goldens/py del manifiesto scripts/ci_python_manifest.json
./scripts/check.sh cov      # cobertura → lcov.info (cargo-llvm-cov)
cargo test --workspace

# Entradas no confiables (requiere nightly + cargo-fuzz)
cargo +nightly fuzz run sav_load
cargo +nightly fuzz run newgrf_parse
cargo +nightly fuzz run net_message
FUZZ_TOOLCHAIN=nightly-2026-07-31 ./scripts/replay_fuzz_regressions.sh  # corpus de PR

# Verificar un paquete extraído sin abrir la ventana
cargo run -- --check-assets

# Validar documentación (enlaces rustdoc, code fences)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

# Auditoría de seguridad y licencias (cargo-audit 0.22.1, cargo-deny 0.20.2)
cargo install cargo-audit --version 0.22.1 --locked  # una vez
cargo install cargo-deny --version 0.20.2 --locked   # una vez
cargo audit            # vulnerabilidades RustSec
cargo deny check       # licencias + advisories + sources + bans (deny.toml)
# Actualizar excepciones: editar deny.toml [advisories].ignore con justificación
```

### Empaquetar el Snap (Linux amd64)

La receta está en [`snap/snapcraft.yaml`](snap/snapcraft.yaml). Construye en
una base `core24` mediante LXD y fija la toolchain Rust 1.98, `clang` y
`mold` para reproducir el perfil de enlace del proyecto:

~~~bash
snapcraft pack --use-lxd --output .
~~~

El resultado se llama `openttdrs_0.1.0-alpha.1_amd64.snap`. Sólo un
mantenedor autenticado debe subir una nueva revisión, después de comprobar el
artefacto:

~~~bash
snapcraft upload --release=latest/edge openttdrs_0.1.0-alpha.1_amd64.snap
~~~

El canal `latest/edge` es la distribución alpha; no publicar desde ese
comando en `stable` sin una decisión de release independiente.

### Caché de compilación (`sccache`)

> Actualizado: 2026-09-19.

GitHub Actions activa `sccache` con el backend de caché de Actions en todos los
jobs que compilan Rust. En local es opcional: `./scripts/check.sh` lo detecta y
lo usa automáticamente, sin hacer que `cargo` directo dependa de una herramienta
extra. Para activarlo también en un comando directo:

```bash
cargo install sccache --locked       # una vez
RUSTC_WRAPPER=sccache cargo build
sccache --show-stats
```

En PowerShell el equivalente es
`$env:RUSTC_WRAPPER = 'sccache'; cargo build`. La caché local queda fuera del
repositorio (por defecto bajo la caché de usuario); CI no comparte artefactos
nativos entre plataformas ni entre compilaciones instrumentadas de cobertura.

| Ruta | Responsabilidad |
|------|-----------------|
| `crates/openttdrs-core/` | Simulación, mapa, comandos, NewGRF parse, save/sav |
| `crates/openttdrs-client/` | Bevy, render, UI, bootstrap |
| `docs/` | Roadmaps e informes — índice: [docs/README.md](docs/README.md) |
| `scripts/` | Assets, `doctor.sh`, `check.sh`, `parse_sav.py`, referencia OpenTTD |
| `tests/fixtures/` | `.sav` + goldens versionados |

**Convención:** lógica de juego en core vía `Command` / `apply_command`; el cliente no mutea el mundo por su cuenta.

Referencia OpenTTD (clon local, no versionado; commit fijado en manifiesto #109):

```bash
./scripts/fetch-openttd-reference.sh   # → reference/openttd-upstream/ @ docs/parity/openttd-reference.json
```

Detalle: [docs/PARIDAD.md](docs/PARIDAD.md).

---

## CI y calidad

> Actualizado: 2026-09-19.

Un job en [.github/workflows/ci.yml](.github/workflows/ci.yml) (sccache + caché Cargo + APT):

| Paso | Contenido |
|------|-----------|
| `rustfmt` | `cargo fmt --all -- --check` |
| `clippy` | workspace, `-D warnings`, perfil `ci` |
| `rustdoc` | `cargo doc` con `-D warnings` (validar enlaces intra-doc) |
| `cargo audit` | Vulnerabilidades RustSec, incluido el lockfile de fuzz (pinned 0.22.1) |
| `cargo deny` | Licencias + advisories + sources + bans, también para fuzz (pinned 0.20.2, `deny.toml`) |
| tests | PRs: `nextest`; push a `main`: `llvm-cov nextest` → Codecov, piso 68% de líneas |
| extras | `tnbp` + `ci-python` (#120) + `generated-tables-ci` (#119) |
| plataformas | `cargo check` en macOS y Windows |
| fuzz | replay determinista en PR + exploración semanal de `.sav`, NewGRF y frames de red |
| release | tag SemVer exacto → prerelease con Linux x86_64, Windows x86_64 y macOS arm64 + SHA-256 |
| Snap | empaquetado manual `core24` validado con lint; revisión 1 publicada en `latest/edge` para Linux amd64 |

`check.sh ci` replica fmt/clippy/rustdoc/tests/TNBP/Python/tablas (hash; regen si hay upstream). Solo en GHA: audit, deny, cobertura en `main` y fetch OpenTTD para regen.

Cobertura manual: [.github/workflows/coverage.yml](.github/workflows/coverage.yml) (`workflow_dispatch`) o `./scripts/check.sh cov`.

### Release alpha

> Actualizado: 2026-09-19.

El tag `v0.1.0-alpha.1` ya produjo la
[prerelease pública](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
mediante [release.yml](.github/workflows/release.yml): binarios, assets libres,
servidor dedicado y checksums SHA-256 para Linux x86_64, Windows x86_64 y macOS
arm64. El workflow también permite comprobar artefactos manualmente sin crear
otro tag.

| Vía | Plataforma | Estado |
|-----|------------|--------|
| [GitHub Releases](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1) | Linux x86_64, Windows x86_64, macOS arm64 | prerelease publicada |
| [Snap Store](https://snapcraft.io/openttdrs) | Linux amd64 | revisión 1, `latest/edge` (alpha) |

El empaquetado local de los paquetes de escritorio equivalente es:

```bash
cargo build --locked --release \
  -p openttdrs-client --bin openttdrs-client \
  -p openttdrs-net --bin openttdrs-dedicated
./scripts/package_release.sh \
  0.1.0-alpha.1 x86_64-unknown-linux-gnu linux-x86_64 tar.gz
```

Para el paquete Snap y su publicación, usar la receta documentada en
[Desarrollo](#empaquetar-el-snap-linux-amd64). Notas:
[CHANGELOG.md](CHANGELOG.md) · [RELEASE_NOTES.md](RELEASE_NOTES.md) ·
[atribuciones de assets](THIRD_PARTY_ASSETS.md).

---

## Qué está hecho / qué falta (resumen)

> Actualizado: 2026-09-19. Las matrices canónicas enlazadas abajo tienen
> prioridad sobre cualquier resumen de esta tabla.

Leyenda: ✅ hecho · 🟡 parcial · ❌ / 🔮 backlog (issues en GitHub)

| Área | Estado | Notas |
|------|--------|-------|
| Construcción road + rail + terraform | ✅ | Waypoints, señales, `RailConvert` (tipo seleccionado) |
| PBS / path signals | 🟡 | Implementado para escenarios acotados; fidelidad global en [PARIDAD.md](docs/PARIDAD.md#estado-canónico-actual) |
| Economía + 11 cargas temperate + packets | 🟡 | CargoDist MCF, transfer/deliver y ratings; climas/NewGRF incompletos |
| Import/export `.sav` | 🟡 | Subconjunto interoperable; matriz única de import vs export en [sav-compatibility.md](docs/parity/sav-compatibility.md) |
| Render OpenGFX vanilla | 🟠 | Cobertura amplia, pero la composición raster global no tiene paridad demostrada; baseline y límites en [PARIDAD.md](docs/PARIDAD.md#evidencia-visual-raster-vigente) |
| UI solitario (menús, listas, noticias) | 🟡 | Jugable; varias opciones del core todavía no están expuestas |
| Multi-compañía | 🟡 | Mínima + ownership; segunda humana OOS |
| NewGRF | 🟡 | Estado por propiedad en la [matriz Action0/3/5](docs/parity/newgrf-action0-matrix.md) y ejecución real en la [matriz de callbacks](docs/parity/newgrf-callback-matrix.md) |
| Barcos | 🟡 | Depósitos, docks, boyas, locks y A*; movimiento/órdenes simplificados |
| Aviones | 🟡 | Airport FTA, compra/vuelo/ruido/crash; render y casos límite incompletos |
| Multijugador (I8) | 🟡 | MVP lockstep + dedicated + host migration; desync/UI OOS |
| IA rivales / GameScript / editor | 🟡 | TransCargo + editor #42 ✅; GS-lite #43 ✅; Squirrel OOS |
| Distribución alpha | ✅ | [GitHub prerelease](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1) + [Snap Store](https://snapcraft.io/openttdrs) `latest/edge` (Linux amd64) |

Al 2026-09-19 no hay [issues activos](https://github.com/cavazquez/openttdrs/issues)
en este corte. Los ajustes P0 #568–#571, la ruta por comandos #572, su
continuación JSON #573, la entrada de menú #574, la guía #575, el feedback de
F5/F9 #576 y el smoke/aceptación de paquete #577 ya fueron entregados.
Los quince anteriores se retiran como `not planned`, no como paridad lograda.
[PARIDAD.md](docs/PARIDAD.md) conserva las capacidades y brechas técnicas.

---

## Documentación

> Actualizado: 2026-09-19.

| Documento | Uso |
|-----------|-----|
| [docs/README.md](docs/README.md) | Índice (un archivo por temática) |
| [docs/PLANIFICACION.md](docs/PLANIFICACION.md) | Roadmaps, sprints y guías de implementación |
| [docs/PARIDAD.md](docs/PARIDAD.md) | Madurez vigente, mapeos, road/rail y oráculos |
| [docs/parity/sav-compatibility.md](docs/parity/sav-compatibility.md) | Fuente única de compatibilidad `.sav` import/export |
| [docs/MAPA_Y_FERROCARRIL.md](docs/MAPA_Y_FERROCARRIL.md) | Formato de mapa, `.ottdmap`, tiles y señales |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Capas + diseño I0–I8 |
| [docs/GRAFICOS.md](docs/GRAFICOS.md) | OpenGFX |
| [docs/RENDIMIENTO.md](docs/RENDIMIENTO.md) | Benches y mapas grandes |
| [snap/snapcraft.yaml](snap/snapcraft.yaml) | Paquete Linux amd64, canal alpha y runtime estricto |

Saves OpenTTD → mapa del cliente:

```bash
python3 scripts/parse_sav.py partida.sav salida.ottdmap
OTTDMAP_FILE=salida.ottdmap cargo run
```

Detalle de planos/chunks: [docs/MAPA_Y_FERROCARRIL.md](docs/MAPA_Y_FERROCARRIL.md#formato-ottdmap). Para regenerar assets de desarrollo: `./scripts/descargar_assets.sh --help`.

---

## Stack

> Actualizado: 2026-09-19.

| Tecnología | Rol |
|------------|-----|
| Rust 2024 (MSRV **1.98**) | Workspace `openttdrs-core` + `openttdrs-client` + `openttdrs-net` |
| Bevy **0.19** | ECS, ventana, render 2D, UI |
| serde / JSON | Save/load del core |
| Python 3 + Pillow | `parse_sav`, goldens, recorte OpenGFX |
| OpenGFX / OpenSFX / OpenMSX | Arte, SFX y música |
| GitHub Actions + Dependabot | CI y deps mensuales |
| Snapcraft 9 + `core24` / LXD | Snap estricto Linux amd64 publicado en `latest/edge` |

---

## Estructura del repo

> Actualizado: 2026-09-19.

```
Cargo.toml                 # Workspace
crates/openttdrs-core/     # Simulación sin Bevy
crates/openttdrs-client/   # Binario Bevy (--server / --client)
crates/openttdrs-net/      # TCP I8 + openttdrs-dedicated
docs/                      # Roadmaps e informes
scripts/                   # check, assets, parse_sav, fetch upstream
snap/                      # receta Snap, launcher y desktop entry
tests/fixtures/            # .sav + goldens
.github/                   # CI + Dependabot
reference/                 # Clon OpenTTD (gitignored)
```

---

## Licencia

> Actualizado: 2026-09-19.

**GPL-2.0-only** (ver `LICENSE`). El código de OpenTTD usado como referencia conserva su propia licencia y copyright.
