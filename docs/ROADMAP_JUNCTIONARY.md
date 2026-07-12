# Roadmap — Junctionary (paridad OpenTTD)

Inventario de **cruces ferroviarios** de la comunidad OpenTTD para implementarlos en openttdrs como escenarios jugables, fixtures de pathfinding/señales y regresión visual.

**Fuente:** [OpenTTD Junctionary](https://wiki.openttd.org/en/Community/Junctionary/) (Community wiki).

**Relacionado:** [SENALES_FERROVIARIAS.md](SENALES_FERROVIARIAS.md), [VIAS_FERROVIARIAS_COLOCACION.md](VIAS_FERROVIARIAS_COLOCACION.md), [parity/scenario.rs](../crates/openttdrs-core/src/parity/scenario.rs), [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) § backlog post-0.1.

---

## Criterio de “implementado” por ítem

Para marcar `[x]` en este roadmap, cada junction debe cumplir **como mínimo**:

| # | Entregable | Dónde |
|---|------------|--------|
| 1 | Mapa reproducible (builder en `parity/scenario.rs` o fixture `.sav` / JSON) | `build_junction_*()` |
| 2 | Tren(es) con órdenes que **atraviesan** todas las ramas usadas en el diseño | `dev_bot` o test de sim |
| 3 | `find_path` / YAPF encuentra ruta entre entradas y salidas del junction | test en `openttdrs-core` |
| 4 | Sin atascos permanentes con señales del tipo que el diseño requiere (bloque, PBS, presignal…) | test ≥ N ticks |
| 5 | (Opcional) Captura `MAP_SHOT` o golden de sprites en pendiente/cruce | cliente |

Los **track layouts** (sin merge/split) son solo geometría + test de colocación de vía.

---

## Fases sugeridas

| Fase | Alcance | Prioridad |
|------|---------|-----------|
| **J0** | Infra: plantilla `build_junction_scenario`, convención nombres, export JSON | Prerrequisito |
| **J1** | 3-way y 4-way básicos + merges + depots/túneles | Alta — enseñan pathfinding |
| **J2** | Cloverleaf, spaghetti, star, roundabout (sin flyover) | Media |
| **J3** | Pre-signaled / RoW / priority merge + layouts | Media — enlaza S5 señales |
| **J4** | Enormous + fly-over/under + dual/quad | Baja — mapas grandes |
| **J5** | “Unuseful” + casos límite (crossover, 3/4) | Baja — regresión |

---

## 3-Way junctions

| Estado | Junction | Fase | Notas |
|--------|----------|------|-------|
| [ ] | Basic 3-Way | J1 | |
| [ ] | Line Merge | J1 | |
| [ ] | Half Cloverleaf | J2 | |
| [ ] | Half Spaghetti | J2 | |
| [ ] | Compact 3-Way | J1 | |
| [ ] | Compact 3-Way B-M Junction | J2 | Branch-Merge |
| [ ] | Ultimate 3-way | J2 | |
| [ ] | Half Transmogrified | J3 | |
| [ ] | Braided Junction | J3 | |

---

## 4-Way junctions

| Estado | Junction | Fase | Notas |
|--------|----------|------|-------|
| [ ] | Basic 4-Way | J1 | |
| [ ] | Roundabout | J2 | |
| [ ] | Cloverleaf | J2 | |
| [ ] | Spaghetti | J2 | |
| [ ] | Star | J2 | |
| [ ] | Complex Star | J3 | |
| [ ] | Branch-Merge | J2 | |
| [ ] | Tetrathorp | J3 | |
| [ ] | High Speed 4-Way Fly-over/under | J4 | Puentes/túneles |
| [ ] | Pre-Signaled Roundabout | J3 | Requiere presignals/PBS |
| [ ] | Advanced Roundabout | J3 | |
| [ ] | Right-of-way Roundabout | J3 | |
| [ ] | Improved Roundabout | J3 | |
| [ ] | Transmogrified | J3 | |
| [ ] | Tightlong | J3 | |
| [ ] | Deepblue2k8 4-way Junction | J4 | |

---

## Enormous junctions

| Estado | Junction | Fase | Notas |
|--------|----------|------|-------|
| [ ] | Multi-way/multi-track cloverleaves | J4 | |
| [ ] | Dual Tetrathorp | J4 | |
| [ ] | Dual Transmogrified | J4 | |
| [ ] | Dual Branch-Merge | J4 | |
| [ ] | Quad Branch-Merge | J4 | |
| [ ] | Quad Tetrathorp | J4 | |
| [ ] | 6-tuple Branch-Merge | J4 | |
| [ ] | 8-Way Star | J4 | |
| [ ] | High Speed 4-way | J4 | |

---

## Other junctions

| Estado | Junction | Fase | Notas |
|--------|----------|------|-------|
| [ ] | Building depots | J1 | Boca depósito + órdenes |
| [ ] | Dual tunnels | J1 | JGR / wormholes si aplica |
| [ ] | Sideline U Turn | J2 | |
| [ ] | Right-of-way Lane Change | J3 | |
| [ ] | Priority Merge | J3 | |

---

## Track layouts

Sin fusiones ni bifurcaciones — solo disposición de vías (paridad de construcción y sprites).

| Estado | Layout | Fase | Notas |
|--------|--------|------|-------|
| [ ] | 4-Way Triple Axial | J3 | |
| [ ] | 4-Way Triple Axial 2 | J3 | |
| [ ] | Triple Via Mini | J3 | |
| [ ] | High Hill | J3 | Pendiente + junctions slope (S3) |

---

## Unuseful junctions

Útiles como **tests de regresión** (pathfinding no debe colgar; señales no deben deadlock eterno).

| Estado | Junction | Fase | Notas |
|--------|----------|------|-------|
| [ ] | Crossover | J5 | |
| [ ] | 3/4ths junction | J5 | |
| [ ] | Ultimate 3-way junction for 4 tracks | J5 | |

---

## Reglas de diseño (wiki → criterios de aceptación)

Extraídas de [Junctionary — Tips](https://wiki.openttd.org/en/Community/Junctionary/) para validar escenarios:

- Curvas de 90° deben tener **≥ 1 tesela recta** entre giros de 45° (velocidad del tren).
- Evitar rampas en el tramo crítico del cruce (o documentar penalización esperada).
- Puentes cortos (&lt; 4 teselas): documentar límite de velocidad si aplica.
- **Merge antes de exit** en el diseño → test debe demostrar que no hay gridlock con ≥ 2 trenes.
- Ramas de salida **más largas que el tren** si hay merge con espera.

---

## Ya en el repo (parcial)

| Escenario | Junction / uso | Gap respecto al Junctionary |
|-----------|----------------|----------------------------|
| `rail_signals_mixed` | Throat presignal + plataformas + línea principal | No es un tipo nombrado del wiki; sirve de J0 demo |
| `train_supply` / `train_supply_dual` | Línea simple / dual carril | Line Merge simplificado |
| `train_supply_signal` | Bloque + señal | Other — señalización básica |

---

## Backlog técnico transversal

- [x] Comando o macro `export_junction_json(name)` para cada escenario
- [x] Catálogo en cliente: `scenario list` / `scenario export` en DevConsole
- [ ] Tabla golden `junctionary_golden.json` con hashes de tiles/señales por diseño
- [x] Documentar en `DEV_BOT.md` flags `--junction <name>` por escenario

---

## Referencias externas

- [Junctionary (wiki)](https://wiki.openttd.org/en/Community/Junctionary/)
- [Owen's — Rail Junctions](https://wiki.openttd.org/) (enlazado desde la wiki)
- [#openttdcoop Junctionary](https://wiki.openttd.org/) (variantes coop)
