# Roadmap global de paridad UI con OpenTTD

Fecha: **2026-07-17**  
Estado: **fuente viva de “siguiente corte” UI** (inventario detallado archivado)

Objetivo: misma **capacidad funcional, descubribilidad y profundidad de navegación**
que OpenTTD; el layout puede diferir si no elimina rutas ni acciones.

## Documentos relacionados

| Documento | Rol |
|-----------|-----|
| [archive/ROADMAP_PARIDAD_UI_GLOBAL_DETAIL.md](archive/ROADMAP_PARIDAD_UI_GLOBAL_DETAIL.md) | Inventario UI-0…UI-8, checklists y baseline histórico |
| [parity/ui_windows_parity.md](parity/ui_windows_parity.md) | Comparación depósito / vehículo / órdenes / horario |
| [ROADMAP_PARIDAD_ESTRUCTURAL.md](ROADMAP_PARIDAD_ESTRUCTURAL.md) | Dependencias de simulación |
| [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md) | Vista corta de gaps |
| [archive/ROADMAP_MENUS_UI.md](archive/ROADMAP_MENUS_UI.md) | Histórico flota |

---

## Definición de paridad (resumen)

Una feature UI alcanza paridad cuando es **descubrible**, **operable** vía
`Command`/`apply_command`, **conserva contexto**, tiene **lifecycle** completo
(setup → Esc / salida InGame) y **pruebas** de apertura/flujo. No basta con un
enum, un dato en el HUD o una hotkey oculta.

---

## Inventario hecho (UI-0…UI-8)

| Fase | Estado | Notas |
|------|--------|-------|
| UI-0 | ✅ | ListWindow, harness, directorios migrados |
| UI-1 | ✅ | MenuSpec, dropdowns, navegación toolbar |
| UI-2 | ✅ | Town / Industry / Station / VehicleList |
| UI-3 | ✅ | Mundo, StationView, subsidios; historial estación opcional |
| UI-4 | ✅ | Flota, refit, shared orders, autoreplace; polish: drag órdenes |
| UI-5 | ✅ | Economía, gráficos, Display Options, mapas |
| UI-6 | ✅ | Construcción jugable (señales, trees, tram, JoinStation MVP, …) |
| UI-7 | ✅ | Settings / NewGRF config-only / ayuda / presets |
| UI-8 | ✅ | Tools-dev, highscore, multi-compañía mínima, Rival IA |

Detalle de criterios y checklists: [archive/…_DETAIL.md](archive/ROADMAP_PARIDAD_UI_GLOBAL_DETAIL.md).

---

## Backlog activo (siguiente corte)

Prioridad tras cierre UI-0…UI-8 (no reabrir fases cerradas salvo regresión):

### P0 — Pulido y huecos jugables

1. ~~Drag nativo de órdenes (pendiente UI-4 polish).~~ ✅ [#194](https://github.com/cavazquez/openttdrs/issues/194)
2. Pulido UI `RailConvert` / ciclo tipo de vía (core existe).
3. Paridad ventanas flota/estación restante — ver [ui_windows_parity.md](parity/ui_windows_parity.md).

### P1 — Sim / red / modding (fuera de UI pura)

1. Desync UI / lobby multijugador ([#21](https://github.com/cavazquez/openttdrs/issues/21) MVP hecho; host migration #171).
2. NewGRF parámetros editables + paridad Action0–14 total → OOS / estructural.
3. LGRJ CargoDist async → OOS.

### P2 — Modos avanzados (no bloquean P0)

1. Editor / GS / IA Squirrel — épicas cerradas en lite; profundidad OOS.
2. Segunda humana local — wontfix (#41); MP humanas → #21.

---

## Arquitectura UI (recordatorio)

- Toolbar → `MenuSpec` / popover → `FloatingWindowId` / `ListWindow`.
- Mutaciones de simulación solo por comandos.
- Single-instance por `FloatingWindowId` (MVP documentado en detalle archivado).

No inventar controles que aparenten funcionar sin backend.

---

## Pruebas mínimas por ventana nueva

- `setup_*`, apertura desde ruta real, `sync_*` vacío/poblado, handlers, cierre/Esc,
  salida/reentrada `InGame`. Preferir `scripts/check.sh`.

---

*Detalle histórico y métricas de cobertura: archive. Actualizar este archivo al
cerrar un corte P0/P1; no volver a inflar con checklists largos.*
