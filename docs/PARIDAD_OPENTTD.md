# Paridad con OpenTTD — vista corta de gaps

Resumen vivo de **openttdrs** vs OpenTTD. Detalle por dominio:

| Tema | Documento |
|------|-----------|
| UI / NewGRF cortes | [ROADMAP_PARIDAD_UI_GLOBAL.md](ROADMAP_PARIDAD_UI_GLOBAL.md) |
| Sim estructural (consist, PBS, economía, railtypes) | [ROADMAP_PARIDAD_ESTRUCTURAL.md](ROADMAP_PARIDAD_ESTRUCTURAL.md) |
| Madurez road / tick | [parity/status.md](parity/status.md) |
| Madurez rail | [parity/rail_status.md](parity/rail_status.md) |
| Sprints 0.1 | [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) |

**Leyenda:** ✅ hecho · 🟡 parcial · ❌ no · 🔮 backlog lejano

---

## Resumen ejecutivo (jul 2026)

| Bloque | Estado |
|--------|--------|
| Carretera + ferrocarril (construcción, sim básica) | ✅ alto |
| Paridad visual OpenGFX vanilla | 🟡 ~85–90 % |
| Audio espacial + música OGG (subset) | 🟡 |
| Economía (préstamos, subsidios, averías, packets) | 🟡 |
| CargoDist MCF nivel 2 | 🟡 (MVP; jobs async OOS) |
| Ciudades (rating, crecimiento) | 🟡 |
| Órdenes y operación de flota | 🟡 |
| Barcos / aviones | 🔮 |
| NewGRF Action0–14 + Action2 runtime | 🟡 (parse + sprites in-world; paridad total OOS) |
| Multijugador I8 | 🟡 MVP ([#21](https://github.com/cavazquez/openttdrs/issues/21) ✅, host migration #171) |
| IA rivales (TransCargo) / GameScript-lite / editor | 🟡 (Squirrel OOS) |

---

## Gaps / mejoras por costo (orden barato → caro)

| # | Área | Feature | openttdrs | Costo | Notas |
|---|------|---------|-----------|-------|-------|
| 1 | Construcción | Convertir tipo de vía | ✅ | S–M | `RailConvert` → tipo seleccionado (#195) |
| 2 | Sim | Servicio en depósito | ❌ | M | |
| 3 | Import/export | `.sav` roundtrip oficial | 🟡 | M | Export propio parcial — [ROADMAP_SAV_EXPORT.md](ROADMAP_SAV_EXPORT.md) |
| 4 | Ferrocarril | PBS golden vs OTTD | 🟡 | L | MVP interno; captura externa |
| 5 | UI | Paridad ventanas flota/estación | 🟡 | M | [ROADMAP_PARIDAD_UI_GLOBAL.md](ROADMAP_PARIDAD_UI_GLOBAL.md) |
| 6 | Mundo | Industrias por clima / gen | 🟡 | L | T1–T3 hechos; T4 parcial |
| 7 | Transporte | Barcos / aviones | 🔮 | L | |
| 8 | Modding | NewGRF paridad total | 🟡 | XXL | Fase 7 MVP en estructural |
| 9 | Red | Desync UI / lobby | 🟡 | XL | Core lockstep hecho |
| 10 | IA | Pathfind construcción / multi-rival | 🟡 | M–L | TransCargo = L Manhattan |

---

## Ya tenemos (alto nivel)

Mapa `.ottdmap`/TNBP · road+rail+señales/PBS MVP · consist · packets/rating · multi-compañía + feeder · IA TransCargo · NewGRF Action0–14 parse + Action2 sprites · I8 TCP + dedicated · save JSON + import/export `.sav` parcial · UI solitario (toolbar, listas, noticias).

---

*Última actualización: 2026-07-17 — vista corta; no duplicar tablas largas aquí.*
