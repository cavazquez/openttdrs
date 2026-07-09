# Paridad con OpenTTD — inventario y costos

Comparación **openttdrs** (hito 0.1) vs **OpenTTD oficial**. Para el plan de trabajo ver
[ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md).

**Leyenda:** ✅ implementado · 🟡 parcial · ❌ no implementado · **Costo:** S = días · M = 1–2 sem · L = 1–2 meses · XL = 3–6 meses · XXL = 6+ meses

---

## Resumen ejecutivo

| Bloque | Estado |
|--------|--------|
| Carretera + ferrocarril (construcción, sim básica) | ✅ alto |
| Paridad visual OpenGFX vanilla | 🟡 ~85–90 % |
| Audio espacial + música OGG (subset) | 🟡 |
| Economía (préstamos, subsidios, averías) | 🟡 |
| Ciudades (rating, crecimiento) | 🟡 |
| Órdenes y operación de flota | 🟡 |
| Barcos / aviones (MVP) | 🟡 |
| Multijugador, NewGRF, Cargo Dist, IA rivales | ❌ |

---

## Tabla por costo de incorporación (gaps y mejoras)

Ordenada de **más barato a más caro**.

| # | Área | Feature | OTTD | openttdrs | Costo |
|---|------|---------|------|-----------|-------|
| 1 | Pulido | Migraciones save JSON | ✅ | ✅ | S |
| 2 | Audio | Música y SFX in-game (`SimEvent`, ~20 SFX, OGG) | ✅ | 🟡 | S |
| 3 | Render | Depósito carretera RemapCoords | ✅ | ✅ | S — `gen_road_depot_gfx_data.py`, tests `iso/mod.rs` |
| 4 | Render | Junctions vía en pendiente | ✅ | 🟡 | S–M |
| 5 | Construcción | Quitar vía (`RailRemove`) | ✅ | ✅ | S |
| 6 | Construcción | Waypoint ferroviario | ✅ | ✅ | S–M |
| 7 | Tests | `effective_road_bits` en fixture | ✅ | ✅ | S |
| 8 | Render | Culling mapas grandes | ✅ | 🟡 | M |
| 9 | Render | Industrias gfx 120–174 | ✅ | 🟡 0–119 | M |
| 9b | Render | Sprites locomotoras por grupo (5 sets OpenGFX) | ✅ | ✅ | S — [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md) § locomotoras |
| 10 | Construcción | Señales bloque (sin PBS) | ✅ | 🟡 render | M |
| 11 | Construcción | Convertir tipo de vía | ✅ | ❌ | M |
| 12 | Import | `.sav` jugable (órdenes, dinero) | ✅ | 🟡 | M |
| 13 | Sim | Servicio en depósito | ✅ | ❌ | M |
| 14 | Sim | Rating estación / cargo packets | ✅ | 🟡 | M |
| 15 | Sim | Órdenes condicionales | ✅ | ✅ | M–L — F6 |
| 16 | Sim | Horarios (timetable) | ✅ | 🟡 | M–L — F1/F4/F5 MVP |
| 16b | Sim | Autoreemplazo | ✅ | 🟡 | M — F2/F3 MVP |
| 17 | Carretera | Un solo sentido / drive-through | ✅ | 🟡 | M |
| 18 | Mundo | Subvencios, autoridad local | ✅ | 🟡 | M–L |
| 19 | Mundo | Terraform + 4 climas + gen mundo | ✅ | 🟡 | T1–T3 + T4 MVP (clima/gen); industrias por clima L — [ROADMAP_TERRAFORM.md](ROADMAP_TERRAFORM.md) |
| 20 | Ferrocarril | PBS + path signals | ✅ | 🟡 | L–XL — PBS completo jugable; presignals ProbeSigSeg v0 + combo chain; falta `_globset` / MultiExit |
| 21 | Economía | Cargo Dist (link graph) | ✅ | ❌ | XL |
| 22 | Transporte | Barcos / aviones | ✅ | 🟡 | L — barcos (canal+esclusa Δh+4 motores); aviones (helipuerto + small 4×3 + fases vuelo) |
| 23 | Modding | NewGRF runtime | ✅ | ❌ | XXL |
| 24 | Red | Multijugador I8 | ✅ | ❌ | XXL |
| 25 | UI | Barra inferior + noticias (ticker, cartel) | ✅ | 🟡 | M — N1–N5 MVP |

---

## Lo que ya tenemos (✅)

| Categoría | Detalle |
|-----------|---------|
| **Mapa** | Teselas `mapt/m1–m8`, `.ottdmap`, TNBP/JGR |
| **Carretera** | Autorail, drag, depósito, túnel, puente, paradas bus/camión |
| **Ferrocarril** | Autorail con curvas/cruce X\|Y, depósito rotado, estación multi-tesela (1–7×1–7), ventana selección, túnel, puente |
| **Pathfinding** | A* carretera; A* direccional vía; invalidación al editar mapa |
| **Industrias** | 10 specs sandbox; producción 256 ticks |
| **Ciudades** | Demanda, etiquetas, ventana pueblo |
| **Vehículos** | Bus, camión, tren; compra/venta; órdenes simples |
| **Economía** | Dinero, costes, inflación, 6 cargos, pago transporte |
| **Save** | JSON versionado (v4) |
| **Import** | `parse_sav.py`, parser Rust, golden CI |
| **Render** | OpenGFX + atlas; terreno, agua, costa, casas, industrias 0–119, estaciones, depósito carretera (RemapCoords) |
| **UI** | Toolbar, minimapa, ventanas flotantes, ghost preview |

---

## Mecánicas iguales vs diferentes

### Parecidas a OpenTTD

- Grilla isométrica y trackbits (X, Y, curvas, cruce X\|Y).
- Autorail por vecinos; giros de tren solo con pieza que conecta.
- Estación tren multi-tesela con layout estándar; cobertura radio 4.
- Tren para en vía adyacente, no sobre plataforma.
- Túneles JGR (wormholes); producción industria 256 ticks.
- Pipeline `.sav` → `.ottdmap` → cliente.

### Simplificadas o distintas

| Mecánica | OpenTTD | openttdrs |
|----------|---------|-----------|
| Tiempo | Calendario + wallclock opcional | Tick numérico |
| Save | Binario comprimido | JSON completo; `.sav` import + export (mapa+STNN+CITY+INDY+VEHS/ORDL+DATE+PLYR) — [ROADMAP_SAV_EXPORT.md](ROADMAP_SAV_EXPORT.md) |
| Pathfinding | YAPF | A* / BFS direccional |
| Señales | PBS, reserva de rutas | Render en mapas importados |
| Cargo | Packets, rating, transit time | Balances `u32` |
| Órdenes | 20+ tipos, condicionales, shared pool | Lista `Station`/`Tile` |
| Vehículos | 4 redes + subtipos | Bus, Truck, Train |
| Multijugador | Replay comandos + desync | No (I8 backlog) |
| Modding | NewGRF runtime | Sprites OpenGFX estáticos |

---

*Última actualización: 2026-06-22*
