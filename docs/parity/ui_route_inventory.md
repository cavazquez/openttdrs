# Inventario de rutas UI (UI-0)

Checklist versionado de superficies de UI. Los conteos deben coincidir con
`FloatingWindowId::ALL` / `BuildMenuAction::ALL` / etc. (test
`ui_enum_inventory_counts`).

**Fecha:** 2026-07-17 · **FloatingWindowId:** 42 · **BuildMenuAction:** 66 ·
**SaveMenuAction:** 22 · **ToolbarGroup:** 8

## Ventanas flotantes (`FloatingWindowId`)

| Id | Apertura típica | Notas |
|----|-----------------|-------|
| Town | clic pueblo / menú | |
| TownDirectory | menú Info / `UiRoute` | |
| IndustryDirectory | menú Info | |
| StationDirectory | menú Info | |
| VehicleList | menú Info / flota | |
| SubsidyList | menú Economía | |
| Depot | clic depósito | |
| BuyVehicle | depósito → comprar | |
| Vehicle | clic vehículo / depósito | Vista (`VehicleView`) |
| VehicleDetails | View → Detalles | Tabs Info/Carga/Capacidad/Totales (#173) |
| RailStationPicker | herramienta estación rail | |
| AirportPicker | herramienta aeropuerto | |
| BridgePicker | tras tramo de puente | |
| DestinationPicker | órdenes → destino | |
| NewsHistory | barra de noticias | |
| Finances | menú Economía | |
| NewsSettings | Ajustes | |
| PathfindingSettings | Ajustes | |
| CargoDistSettings | Ajustes | Manual / Asimétrica / Simétrica |
| AiSettings | Ajustes / Finanzas «IA…» | |
| NewGrf | Ajustes | |
| SoundMusic | toolbar audio | |
| Timetable | vehículo / F4 | |
| Orders | View → Órdenes / estación (#176) | Flotante; ya no dock fijo |
| Refit | depósito | |
| SharedOrders | vehículo | |
| Autoreplace | depósito / flota | |
| Graphs | menú Economía | |
| CargoPaymentRates | menú Economía | |
| DisplayOptions | Ajustes | |
| ExtraViewport | Ajustes | |
| SignList | menú Info | |
| LinkGraphLegend | menú Economía | |
| SignalPicker | herramienta señales | |
| Help | Ajustes / F1 | |
| DevConsole | Ajustes / F3 | |
| TileInspector | Ajustes / F2 | |
| CheatWindow | Ajustes / Ctrl+Alt+C | |
| GenLand | Editor → Terreno | |
| Goals | menú Economía | |
| Story | menú Mundo | |
| League | menú Economía | |

## Paneles no flotantes (fijos)

| Superficie | Apertura |
|------------|----------|
| StationCargoPanel | clic estación |
| IndustryPanel | clic industria |
| SaveWindow | Guardar/Cargar |
| Minimap | HUD |
| Build toolbar groups | barra superior |

## Toolbar

- **ToolbarGroup (8):** Rail, Road, Water, Air, Economy, Landscape, Info, Settings
- **BuildMenuAction (66):** ver `BuildMenuAction::ALL` en `toolbar/mod.rs`
- **SaveMenuAction (18):** ver `SaveMenuAction::ALL`

## Mantenimiento

1. Añadir variante al enum.
2. Actualizar `ALL` y este checklist.
3. Ajustar constantes en `ui_enum_inventory_test.rs`.
4. `cargo test -p openttdrs-client --bin openttdrs-client ui_enum_inventory`.
