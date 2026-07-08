# Documentación — openttdrs

Índice de la carpeta `docs/`. Para empezar a desarrollar o planificar, leer en este orden:

1. [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) — plan de trabajo actual (6 sprints, hito 0.1)
2. [ROADMAP_MENUS_UI.md](ROADMAP_MENUS_UI.md) — **menús de flota** (órdenes, vehículo, depósito): handoff para IAs
3. [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) — qué tenemos vs OpenTTD y costo de cada gap
4. [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md) — hallazgos técnicos fijos y comandos útiles
5. [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) — filosofía I0–I8 y estado del código

---

## Planificación y producto

| Documento | Uso |
|-----------|-----|
| [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md) | Sprints S1–S6, criterios de cierre 0.1 |
| [ROADMAP_IMPORTACION_OPENTTD.md](ROADMAP_IMPORTACION_OPENTTD.md) | **Importación desde original:** animaciones, sonido, música, dinámicas (tablas + orden sugerido) |
| [ROADMAP_MENUS_UI.md](ROADMAP_MENUS_UI.md) | Menús flota (vehículo, órdenes, depósito), comandos, gaps OTTD, handoff IA |
| [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) | Inventario features + mecánicas iguales/diferentes |
| [DISENO_INCREMENTAL.md](DISENO_INCREMENTAL.md) | Spec incremental I0–I8, traducción upstream ↔ Rust |
| [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md) | Prioridades, hallazgos de tiles/sprites, comandos |
| [ROADMAP_PARIDAD_VISUAL.md](ROADMAP_PARIDAD_VISUAL.md) | Checklist visual vs OpenTTD 15.3 (mayoría [x]) |
| [ROADMAP_TERRAFORM.md](ROADMAP_TERRAFORM.md) | Elevar / bajar / nivelar terreno (T1–T3) |
| [ROADMAP_NEWS_STATUSBAR.md](ROADMAP_NEWS_STATUSBAR.md) | Barra inferior, ticker y cartel de noticias (N1–N5) |
| [ROADMAP_CARRETERAS_DRAG.md](ROADMAP_CARRETERAS_DRAG.md) | **Handoff:** construcción carretera drag/orientación (bug abierto) |
| [HANDOFF_BUGS_VISUALES_TERRAIN.md](HANDOFF_BUGS_VISUALES_TERRAIN.md) | **Handoff:** teselas oscuras, ghost al iniciar, casas Toyland, densidad hierba (jul 2026) |
| [DEV_BOT.md](DEV_BOT.md) | **DevBot:** sonda headless carga/descarga/ingresos — comandos listos |
| [epics/ai_rivals.md](epics/ai_rivals.md) | Épica futura: IA rivales CPU |
| [parity/RAIL_REVIEW_HANDOFF.md](parity/RAIL_REVIEW_HANDOFF.md) | **Handoff IA avanzada:** revisión post Rail 0–4 (paridad ferroviaria) |
| [ROADMAP_MAIN_MENU.md](ROADMAP_MAIN_MENU.md) | Menú inicio — pantallas, Nueva partida procedural, cargar, intro animada (fase 2) |

### Organización del código (jun 2026)

Módulos grandes partidos siguiendo el patrón `ui/save_window/`:

| Área | Ruta |
|------|------|
| Menú inicio | `crates/openttdrs-client/src/ui/main_menu/` |
| Flota (vehículo, órdenes, depósito) | `ui/vehicle_window.rs`, `ui/toolbar/order_panel/`, `ui/toolbar/depot_panel.rs` — ver [ROADMAP_MENUS_UI.md](ROADMAP_MENUS_UI.md) |
| Población procedural | `crates/openttdrs-client/src/state/bootstrap/procedural_population/` |
| Comandos transporte | `crates/openttdrs-core/src/command/transport/` |
| Tests de command | `crates/openttdrs-core/src/command/tests/` |

Las APIs públicas (`ui.rs`, `command/mod.rs`, `bootstrap/world.rs`) no cambiaron de imports.

---

## Mapa, saves y flujo de datos

| Documento | Uso |
|-----------|-----|
| [FLUJO_MAPA_Y_CLIENTE.md](FLUJO_MAPA_Y_CLIENTE.md) | Save → `.ottdmap` → cliente → JSON |
| [OTTDMAP_FORMAT.md](OTTDMAP_FORMAT.md) | Especificación binaria `.ottdmap` |
| [TILES_Y_SAVEGAMES_OPENTTD.md](TILES_Y_SAVEGAMES_OPENTTD.md) | MAPT, `m5`, semántica tiles OpenTTD |
| [SENALES_FERROVIARIAS.md](SENALES_FERROVIARIAS.md) | Tipos de señal OpenTTD, codificación mapa, plan de implementación |
| [VIAS_FERROVIARIAS_COLOCACION.md](VIAS_FERROVIARIAS_COLOCACION.md) | Horz/Vert/X/Y vs autoraíl, uniones, pick de señales |
| [SNAPSHOT_ORACLE_WORKFLOW.md](SNAPSHOT_ORACLE_WORKFLOW.md) | Comparación manual con fork OpenTTD oráculo |

---

## Gráficos y render

| Documento | Uso |
|-----------|-----|
| [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md) | Guía práctica: NFO, extracción, proyección isométrica; **§ locomotoras** (gap visual trenes) |
| [SPRITES_OPENGFX_COMPLETO.md](SPRITES_OPENGFX_COMPLETO.md) | Catálogo de IDs de sprite (referencia) |
| [ROADMAP_INDUSTRIAS_PARIDAD.md](ROADMAP_INDUSTRIAS_PARIDAD.md) | Paridad industrias gfx 0–174 |
| [INDUSTRIAS_OPENGFX.md](INDUSTRIAS_OPENGFX.md) | Tabla gfx → sprite_id por industria |
| [SP3_AUDIT_SUMMARY.md](SP3_AUDIT_SUMMARY.md) | Auditoría de assets PNG y fixtures visuales |

---

## Construcción y regresión (SP2 cerrado)

| Documento | Uso |
|-----------|-----|
| [SP2_CHECKLIST.md](SP2_CHECKLIST.md) | Checklist manual + automático construcción |
| [SP2_PARADAS_Y_ESTACIONES.md](SP2_PARADAS_Y_ESTACIONES.md) | Paradas bus/camión/tren, sprites, conexión carretera |

---

## Referencia upstream

| Documento | Uso |
|-----------|-----|
| [INFORME_ARQUITECTURA_OPENTTD.md](INFORME_ARQUITECTURA_OPENTTD.md) | Arquitectura OpenTTD C++ (clon local) |

---

## Archivo histórico

Planes y notas de sesión **cerrados o absorbidos en código** — conservados por contexto, no son la fuente de verdad actual:

| Documento | Motivo del archivo |
|-----------|-------------------|
| [archive/PLAN_SP2_CONSTRUCCION.md](archive/PLAN_SP2_CONSTRUCCION.md) | SP2 cerrado 2026-05-22 |
| [archive/PLAN_SP4_PULIDO.md](archive/PLAN_SP4_PULIDO.md) | Sustituido por ROADMAP_SPRINTS S1 |
| [archive/PLAN_SP3_VISUAL.md](archive/PLAN_SP3_VISUAL.md) | Fases SP3.0–3.6 cerradas; huecos en ROADMAP_SPRINTS S3 |
| [archive/PLAN_SP3_CASAS_INDUSTRIAS.md](archive/PLAN_SP3_CASAS_INDUSTRIAS.md) | P1–P6 cerrados |
| [archive/PLAN_PARADAS_REMAPCOORDS.md](archive/PLAN_PARADAS_REMAPCOORDS.md) | Implementado en código |
| [archive/PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md](archive/PLAN_DEPOSITO_CARRETERA_REMAPCOORDS.md) | Implementado (jun 2026) — histórico |
| [archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md](archive/SESION_OTTDMAP_SIGNALS_SIM_2026-04-28.md) | Nota de sesión; integrado en core/cliente |
| [archive/SESION_CLIENTE_MAPA_COSTA_2026-04-28.md](archive/SESION_CLIENTE_MAPA_COSTA_2026-04-28.md) | Nota de sesión costa/agua |

Ver [archive/README.md](archive/README.md).

---

*Última actualización: 2026-06-22 (refactor módulos, menú fase 2, CI nextest)*
