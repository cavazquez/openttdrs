# Roadmap: paridad estructural vs OpenTTD

Plan vivo para cerrar las divergencias estructurales. Fases **1–3** (consist,
packets, YAPF/PBS MVP) implementadas jul 2026. Fases 4–7 = hitos 0.4–0.6.

## Fase 1 — Consist / vagones ✅

- Modelo `next_unit` / `prev_unit`, `cached_total_length`, potencia/peso agregados.
- Catálogo: `ENGINE_WAGON_*` (150–153).
- Comandos: `AttachWagonToConsist`, `DetachConsistUnit`, `MoveRailVehicle`, venta de cadena.
- Save JSON v12 + migración; import `.sav` conserva vagones.
- UI: compra auto-engancha; depósito muestra `[Nu]`; render de trailers.

## Fase 2 — Cargo packets + rating + carga gradual ✅

- `cargo_packet.rs`: `CargoPacket`, colas estación/vehículo, `load_unload_speed`.
- Carga/descarga gradual por tick; pago por packet (`periods_in_transit`).
- Rating combina `time_since_pickup` + edad del packet más viejo.
- Save JSON v13 + migración desde balances; UI estación/vehículo muestra rating/packets.
- Pendiente: tabla temperate completa (11 cargos), decay fuerte, feeder_share.

## Fase 3 — YAPF incremental + PBS paridad ✅ (MVP)

- YAPF: `next_rail_trackdir_yapf` / `extend_rail_path_yapf`; búsqueda trackdir.
- PBS: huella multi-tesela del consist (`rail_tile_history` + `consist_occupied_tiles`).
- Reserva de plataforma en orden Station; TryReserve con wormholes JGR.
- Pendiente: golden tick-a-tick vs OpenTTD; FollowTrainReservation fino.

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
