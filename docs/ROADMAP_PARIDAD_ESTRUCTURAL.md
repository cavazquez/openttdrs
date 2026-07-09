# Roadmap: paridad estructural vs OpenTTD

Plan vivo para cerrar las divergencias estructurales. **Fase 1 (consist)**
implementada jul 2026. Fases 2–7 quedan como hitos 0.2–0.6.

## Fase 1 — Consist / vagones ✅

- Modelo `next_unit` / `prev_unit`, `cached_total_length`, potencia/peso agregados.
- Catálogo: `ENGINE_WAGON_*` (150–153).
- Comandos: `AttachWagonToConsist`, `DetachConsistUnit`, `MoveRailVehicle`, venta de cadena.
- Save JSON v12 + migración; import `.sav` conserva vagones.
- UI: compra auto-engancha; depósito muestra `[Nu]`; render de trailers.

## Fase 2 — Cargo packets + rating + carga gradual

Reemplazar `CargoStock` u32 por packets con origen/edad; `LoadUnloadVehicle`
gradual; rating de estación. Ampliar tabla de cargos temperate.

## Fase 3 — YAPF incremental + PBS paridad

Pathfinding por trackdir; PBS multi-tesela del consist; reservas de plataforma
y túnel/puente. Golden vs OpenTTD.

## Fase 4 — Economía multi-compañía + Cargo Dist + IA

Pool de compañías; link graph; IA rivales básicas.

## Fase 5 — Railtypes / electrificación / RailConvert

`RailTypeInfo`, compatibilidad motor↔vía, toolbar `RailConvert`, catenaria.

## Fase 6 — Tranvía / monorail / maglev

Sistemas propios (bits, pathfinding, depósitos) sobre railtypes.

## Fase 7 — NewGRF runtime completo

Loader + Action0–14, callbacks, gfx ≥175, roundtrip save con GRF activos.

## UI continua

Paridad de ventanas según [`parity/ui_windows_parity.md`](parity/ui_windows_parity.md)
en cada fase (depósito drag, rating, overlay PBS, compañías, NewGRF GUI).
