# openttdrs

Port **incremental** de ideas y mecánicas inspiradas en [OpenTTD](https://www.openttd.org/) hacia **Rust**, con motor gráfico [Bevy](https://bevyengine.org/). El objetivo a largo plazo es un simulador modular; la **paridad total** (NewGRF, red, saves idénticos) es un alcance opcional y costoso en tiempo.

> **Rendimiento en tu máquina:** compilar Bevy y dependencias puede ser pesado. Si notas saturación de CPU o RAM, usa por ejemplo `cargo build -j 1` o deja que el flujo de [CI](.github/workflows/ci.yml) valide el build en GitHub Actions.

**Roadmap en issues:** tabla, dificultades y grafo de dependencias en [docs/ISSUES_DEPENDENCY_GRAPH.md](docs/ISSUES_DEPENDENCY_GRAPH.md) (issues [#2](https://github.com/cavazquez/openttdrs/issues/2)–[#13](https://github.com/cavazquez/openttdrs/issues/13) en este repo).

---

## Stack tecnológico

| Tecnología | Descripción |
|------------|-------------|
| 🦀 [Rust](https://www.rust-lang.org/) | Lenguaje y toolchain; edición **2024** en el workspace. |
| 📦 [Cargo](https://doc.rust-lang.org/cargo/) | Workspace con crates `openttdrs-core` y `openttdrs-client`. |
| 🎮 [Bevy](https://bevyengine.org/) | Motor ECS, ventana, cámara 2D, gizmos de depuración (cliente). |
| 🖼️ [wgpu](https://wgpu.rs/) (vía Bevy) | API gráfica usada por debajo del render de Bevy. |
| 🧪 Tests + clippy | Calidad en `openttdrs-core`; CI en GitHub. |
| ✅ [GitHub Actions](https://docs.github.com/en/actions) | Workflow `ci.yml`: `fmt`, `clippy`, `test`, `build`. |
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
```

---

## Estructura del repo

```
Cargo.toml                 # Workspace
rust-toolchain.toml
crates/openttdrs-core/     # Mapa, tick, estado sin Bevy
crates/openttdrs-client/   # Binario Bevy
docs/                      # Informe de arquitectura upstream
scripts/                   # fetch-openttd-reference.sh
.github/                   # CI + Dependabot
reference/                 # Clon local ignorado por git
```

---

## Licencia

El proyecto openttdrs se distribuye bajo **GPL-2.0** (ver archivo `LICENSE`). El código de OpenTTD usado como referencia mantiene su propia licencia y copyright; no asumas compatibilidad con otras licencias sin revisión explícita.
