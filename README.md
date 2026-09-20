# openttdrs

<p align="center">
  <img src="static/app/openttdrs-icon.png" alt="openttdrs" width="220">
  <br>
  <a href="https://snapcraft.io/openttdrs">
    <img src="https://snapcraft.io/static/images/badges/en/snap-store-black.svg" alt="Get it from the Snap Store" width="180">
  </a>
</p>

[![CI](https://github.com/cavazquez/openttdrs/actions/workflows/ci.yml/badge.svg)](https://github.com/cavazquez/openttdrs/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/cavazquez/openttdrs/graph/badge.svg)](https://codecov.io/gh/cavazquez/openttdrs)
[![Licencia GPL-2.0-only](https://img.shields.io/badge/licencia-GPL--2.0--only-blue.svg)](LICENSE)
[![Rust MSRV](https://img.shields.io/badge/rust-1.98%2B-orange.svg)](https://doc.rust-lang.org/stable/releases.html)
[![Bevy](https://img.shields.io/badge/Bevy-0.19.0-C659D4.svg)](https://bevyengine.org/)
[![Snap Store](https://snapcraft.io/openttdrs/badge.svg)](https://snapcraft.io/openttdrs)
[![Inspiración OpenTTD](https://img.shields.io/badge/inspiración-OpenTTD-5a3.svg)](https://www.openttd.org/)

Simulador de transporte inspirado en [OpenTTD](https://www.openttd.org/), escrito en **Rust** con cliente [Bevy](https://bevyengine.org/). El desarrollo es **incremental**: siempre hay algo jugable; la paridad total (NewGRF completo, red, saves idénticos al original) se aborda por cortes documentados, no de golpe.

La alpha pública de escritorio actual es [`0.1.0-alpha.1`](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1). La revisión 1 del [Snap para Linux amd64](https://snapcraft.io/openttdrs) en `latest/edge` tiene un defecto de arranque: intenta materializar tiles OpenGFX dentro de `$SNAP`, que es de sólo lectura. [#582](https://github.com/cavazquez/openttdrs/issues/582) y [#583](https://github.com/cavazquez/openttdrs/issues/583) preparan y validan `0.1.0-alpha.2`; hasta entonces, usar los paquetes de escritorio de GitHub. Es un canal de pruebas, no una promesa de estabilidad ni de paridad con OpenTTD.

> Compilar Bevy puede saturar CPU/RAM. Si hace falta: `cargo build -j 1`, o dejá que [CI](.github/workflows/ci.yml) valide el build. Las ejecuciones repetidas de `./scripts/check.sh` aprovechan `sccache` automáticamente cuando está instalado.

**Gobierno:** [CONTRIBUTING.md](CONTRIBUTING.md) · [SECURITY.md](SECURITY.md) · [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) · [ADRs](docs/adr/)

**Última actualización:** 2026-09-20

---

## Estado del proyecto

> Dirección actualizada: 2026-09-20. Las capacidades técnicas conservan sus
> matrices de evidencia; completar este corte no certifica paridad global.

**Estado jugable actual.** El menú ofrece nueva partida, carga, escenarios o
heightmaps, editor y demo. La alpha permite construir y operar servicios,
cobrar, guardar y reanudar en mapas procedurales con OpenGFX y ES/EN. Las
validaciones de carretera por comandos y continuación JSON siguen como fixtures
internos; ya no se presentan como un modo o guía independiente. [#576](https://github.com/cavazquez/openttdrs/issues/576)
añadió feedback visible de F5/F9 y [#577](https://github.com/cavazquez/openttdrs/issues/577)
certificó el arranque gráfico del paquete Linux extraído fuera del checkout. Ese
smoke no emulaba el mount read-only de Snap; [#582](https://github.com/cavazquez/openttdrs/issues/582) y [#583](https://github.com/cavazquez/openttdrs/issues/583) corrigen y cubren esa diferencia. El
[plan ejecutable](docs/parity/continuous-work-plan.md) conserva la evidencia
del corte; la [auditoría](docs/audits/2026-09-18-direction.md) explica la
selección original. La distribución alpha publicada está disponible como
[prerelease de GitHub](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1)
y [Snap Store `latest/edge`](https://snapcraft.io/openttdrs).

| Capa | Qué hay |
|------|---------|
| **Core** (`openttdrs-core`) | Mapa, tick, comandos, simulación road/rail, señales/PBS parcial, economía, saves JSON + import/export `.sav` / `.ottdmap`; alcance `.sav` en la [matriz canónica](docs/parity/sav-compatibility.md) |
| **Cliente** (`openttdrs-client`) | Vista isométrica OpenGFX, menú de inicio, toolbar, listas UI, noticias; `--server` / `--client` (I8) |
| **Red** (`openttdrs-net`) | TCP lockstep + bin `openttdrs-dedicated` ([ADR 0001](docs/adr/0001-multiplayer-v1.md)) |
| **NewGRF** | Catálogos Action0/3/5 y runtime parcial; las matrices de [propiedades](docs/parity/newgrf-action0-matrix.md) y [callbacks](docs/parity/newgrf-callback-matrix.md) distinguen parseado, almacenado y ejecutado |
| **Hito 0.1** | [`0.1.0-alpha.1` publicada](https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1); solitario jugable en paquetes de escritorio. El Snap `latest/edge` revisión 1 está en corrección de arranque ([#582](https://github.com/cavazquez/openttdrs/issues/582), [#583](https://github.com/cavazquez/openttdrs/issues/583)). **I8 red** MVP ([#21](https://github.com/cavazquez/openttdrs/issues/21) ✅) + host migration ([#171](https://github.com/cavazquez/openttdrs/issues/171), [ADR 0004](docs/adr/0004-host-migration-post-v1.md)) |

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

En Linux **amd64**, el [Snap Store](https://snapcraft.io/openttdrs) volverá a
ser la vía más corta al publicarse `0.1.0-alpha.2`. La revisión 1 vigente en
`latest/edge` tiene el defecto de arranque documentado en [#582](https://github.com/cavazquez/openttdrs/issues/582), por lo que no debe usarse como instalación jugable. Tras la corrección, el comando será:

~~~bash
sudo snap install openttdrs --channel=latest/edge
openttdrs
~~~

`latest/edge` es el canal alpha del proyecto. El Snap corregido será estricto,
incluirá los assets libres y guardará sus partidas JSON por defecto en
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

Eso abre el menú con **Nueva partida**, **Cargar partida**, escenarios, editor,
demo y salida. En el primer inicio se crean bajo `assets/opengfx/tiles/` los PNG derivados del atlas
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
# Comprobar el .snap recién construido sin instalarlo en el perfil real:
./scripts/smoke_snap_package.sh openttdrs_<version>_amd64.snap
~~~

El nombre exacto usa el campo `version` de la receta, por ejemplo
`openttdrs_0.1.0-alpha.2_amd64.snap`. El smoke desmonta el contenido en un
directorio temporal, lo ejecuta con `$SNAP` de sólo lectura y usa un perfil
efímero: así detecta assets que el paquete intentaría generar en runtime. Sólo
un mantenedor autenticado debe subir una nueva revisión, después de comprobar
el artefacto:

~~~bash
snapcraft upload --release=latest/edge openttdrs_<version>_amd64.snap
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
| Snap | empaquetado manual `core24`; la revisión 1 publicada en `latest/edge` tiene un bloqueo de arranque. `0.1.0-alpha.2` añade materialización en build y smoke read-only ([#582](https://github.com/cavazquez/openttdrs/issues/582), [#583](https://github.com/cavazquez/openttdrs/issues/583)) |

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
| [Snap Store](https://snapcraft.io/openttdrs) | Linux amd64 | revisión 1 en `latest/edge`, con bloqueo de arranque conocido; `0.1.0-alpha.2` en preparación ([#582](https://github.com/cavazquez/openttdrs/issues/582), [#583](https://github.com/cavazquez/openttdrs/issues/583)) |

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

> Corte V1: 2026-09-20, base inspeccionada `72d906d8`. ✅ significa que pasa
> **el contrato acotado de esta fila**, no paridad completa con OpenTTD.
> 🟡 significa que falta implementación o evidencia de aceptación.

El [contrato V1](docs/parity/acceptance-v1.md) fija fixtures, tolerancias y
exclusiones antes de implementar. El objetivo es llevar todas estas filas a
verde con pruebas reproducibles. Las [matrices canónicas](docs/PARIDAD.md)
siguen registrando las brechas generales aunque un caso V1 esté aprobado.

| Área | Hecho hoy | V1 | Contrato de cierre / pendiente atómico |
|------|-----------|----|---------------------------------------|
| Construcción road + rail + terraform básicos | Comandos, conexiones, depósitos y cambios de terreno | ✅ | 75 regresiones de construcción verificadas; no certifica todas las geometrías |
| PBS / path signals | Reservas y movimiento con oracle externo | ✅ | Fixtures simple, dual-curva y consist de 3 unidades: 14 tests; ventanas de 40/400/500 ticks y caso rico de 2.000 ticks |
| Ruta vial + persistencia JSON | Construcción por comandos, carga, pago, guardar y continuar | ✅ | 2 tests: entrega antes de 40.000 ticks y continuación exacta durante 2.000 ticks |
| Economía Temperate + packets | 11 cargas, pagos, transfer/deliver, ratings y CargoDist | 🟡 | 198 pagos exactos [#586](https://github.com/cavazquez/openttdrs/issues/586) ✅ ([oráculo](docs/parity/temperate-payment-oracle.md) nativo) y transferencia de carbón en dos tramos [#587](https://github.com/cavazquez/openttdrs/issues/587) ✅ ([traza](docs/parity/coal-transfer-v1.md), conservación y feeder); ambos esperan CI remota vigente antes del cierre |
| Import/export `.sav` | [Subconjunto interoperable](docs/parity/sav-compatibility.md); gate de 6 cargas y un roundtrip | 🟡 | [#588](https://github.com/cavazquez/openttdrs/issues/588) implementa una edición ORDL pública y exacta tras re-guardado nativo ([evidencia](docs/parity/sav-ordl-v1.md)); espera CI remota vigente, no SAV universal |
| Render OpenGFX vanilla | Sprites y compositor amplios; framebuffer global todavía divergente | ✅ | [#589](https://github.com/cavazquez/openttdrs/issues/589) certifica Kale `(132,2)` Normal: 437/480 píxeles, 5/24 >64, media 0,02109/0,05 y cobertura 0 ([evidencia](docs/parity/raster-v1-kale.md)); los otros zooms siguen diagnósticos y el cierre espera CI remota vigente |
| UI solitario | Menú ES/EN, ventanas y feedback F5/F9 | ✅ | Gate de certificación separado y presupuesto fijo [#584](https://github.com/cavazquez/openttdrs/issues/584) ✅; [#590](https://github.com/cavazquez/openttdrs/issues/590) certifica edición de Órdenes en 8 perfiles con 0 píxeles fuera de presupuesto ([evidencia](docs/parity/orders-v1.md)). Quedan otras ventanas/opciones fuera de este corte |
| Multi-compañía | Ownership y asignación de compañía por cliente | 🟡 | [#591](https://github.com/cavazquez/openttdrs/issues/591) implementado: 4 rechazos atómicos sobre bienes ajenos, 4 controles válidos de A y issuer inválido ([evidencia](docs/parity/ownership-v1.md)); falta CI remota vigente |
| NewGRF | [Action0/3/5](docs/parity/newgrf-action0-matrix.md) y [callbacks runtime](docs/parity/newgrf-callback-matrix.md) parciales | 🟡 | Un GRF de camión con CB36, catálogo y continuación JSON de 2.000 ticks [#592](https://github.com/cavazquez/openttdrs/issues/592) |
| Barcos | Depósitos, docks, boyas, esclusas y controlador naval | 🟡 | Compra → boya → entrega pagada en una ruta marítima [#593](https://github.com/cavazquez/openttdrs/issues/593); quedan fuera canales/locks/YAPF global |
| Aviones | FTA, compra/vuelo y oracle Helidepot (4 tests verificados) | 🟡 | Servicio pagado de un avión entre dos aeropuertos Country [#594](https://github.com/cavazquez/openttdrs/issues/594); no todos los layouts |
| Multijugador propio | TCP lockstep, dedicated, late join, resync y host migration | 🟡 | Gate TCP obligatorio [#585](https://github.com/cavazquez/openttdrs/issues/585) ✅ (`PermissionDenied` falla; inyección negativa + 22 recorridos loopback, 0 omitidas); faltan 2 clientes durante 2.000 ticks [#595](https://github.com/cavazquez/openttdrs/issues/595); sin protocolo OpenTTD |
| IA rivales propias | TransCargo y RoadHaul construyen rutas | 🟡 | [TransCargo #596](https://github.com/cavazquez/openttdrs/issues/596) y [RoadHaul #597](https://github.com/cavazquez/openttdrs/issues/597) implementados: entregas físicas, compra/órdenes y saldo humano aislado ([evidencia ferroviaria](docs/parity/transcargo-v1.md), [evidencia de pasajeros](docs/parity/roadhaul-v1.md)); falta CI remota vigente. No NoAI/Squirrel |
| GS-lite propio | Goals, story y league; `CargoDelivered` cuenta sólo las unidades finales del tipo solicitado por la compañía | 🟡 | [#598](https://github.com/cavazquez/openttdrs/issues/598) implementado con ledger persistente por cargo y [evidencia](docs/parity/gs-cargo-filter-v1.md); falta CI remota vigente y conservar progreso/noticia tras JSON [#599](https://github.com/cavazquez/openttdrs/issues/599); no GameScript/Squirrel |
| Editor de escenarios | Herramientas, guardado y apertura propios | 🟡 | Editar → guardar → abrir para jugar un escenario 64×64 [#600](https://github.com/cavazquez/openttdrs/issues/600); no `.scn` universal |
| Distribución alpha | Prerelease multiplataforma publicada; fix y smoke Snap read-only locales | 🟡 | Instalar/validar Snap [#582](https://github.com/cavazquez/openttdrs/issues/582)/[#583](https://github.com/cavazquez/openttdrs/issues/583), menú gráfico [Windows #601](https://github.com/cavazquez/openttdrs/issues/601) y [macOS #602](https://github.com/cavazquez/openttdrs/issues/602) |
| Certificación del corte | Tests y reportes parciales existentes | 🟡 | Un resultado por contrato y SHA, sin `skip` convertido en éxito [#603](https://github.com/cavazquez/openttdrs/issues/603) |

Tolerancias V1 vinculantes; cada fila verde conserva su evidencia del mismo
candidato y las demás siguen pendientes de instrumentación donde lo indican los issues:

- **Estado, dinero, carga, órdenes, RNG y ownership:** cero diferencias en los
  casos declarados. Servicio pagado: hasta 40.000 ticks; continuación: 2.000.
- **Raster de la escena fijada:** ≤0,1 % de píxeles distintos, ≤0,005 % con
  delta de canal >64, media por canal ≤0,05/255 y cero cobertura faltante.
  Sin desplazar ni recortar la comparación para aprobar.
- **Regresión de UI propia:** por región fija, ≤0,5 % de píxeles con delta
  >8/255 y media ≤1/255; cero controles/textos requeridos ausentes. Esto no
  certifica semejanza con la UI de OpenTTD.

Los 20 issues nuevos **#584–#603** y los dos existentes **#582/#583** forman
el [backlog ejecutable](docs/parity/continuous-work-plan.md). Los quince padres
históricos siguen retirados como `not planned`, nunca como paridad lograda.
No se declara CI completa verde ni se cambian goldens para aprobar este resumen.

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
| [scripts/smoke_snap_package.sh](scripts/smoke_snap_package.sh) | Smoke de un `.snap` extraído con `$SNAP` de sólo lectura |

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
| Snapcraft 9 + `core24` / LXD | Snap estricto Linux amd64; revisión 1 en corrección de arranque antes de la próxima alpha |

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
