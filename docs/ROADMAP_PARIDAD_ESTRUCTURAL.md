# Roadmap: paridad estructural vs OpenTTD

Plan vivo para cerrar las divergencias estructurales. Fases **1–6** MVP
implementadas jul 2026. Fase 7 = hito 0.6 (MVP parcial + runtime completo pendiente).

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

## Fase 4 — Economía multi-compañía + Cargo Dist + IA ✅ (MVP)

- Pool `companies` + `CompanyId`; `owner` en vehículo/estación; save **v14**.
- Feeder share 25 % vía `first_station` (sin link graph completo).
- IA rival «TransCargo» (`ai/rule_based.rs`); escenario `ai_rival_line`.
- Pendiente: link graph / Cargo Dist real; UI selector multi-compañía; quiebra rival.

## Fase 5 — Railtypes / electrificación / RailConvert ✅ (MVP)

- `rail_type.rs`: `Rail` / `Electric` en `m8` bits 0–5; `current_rail_type`.
- `Command::ConvertRail`; eléctricos (110–113) exigen vía electrificada adyacente.
- Toolbar «Convertir vía»; save **v15**.
- Catenaria Action5 (`ogfxe_extra`): wires + postes PPP + entradas túnel;
  PCP/PPP por vecinos; vanos de puente; `OPENTTDRS_HIDE_CATENARY` /
  `OPENTTDRS_TRANSPARENT_CATENARY` (TO_CATENARY MVP).
- Pendiente: UI de transparencia, estaciones CanStationTileHavePylons/Wires.

## Fase 6 — Monorail / maglev ✅ (MVP)

- `RailType::{Monorail, Maglev}` (2/3); redes aisladas (solo Rail↔Electric compatibles).
- Motores X2001 (154) / Lev1 (184); compra exige vía adyacente del tipo.
- YAPF filtra por `required_rail_type_for_engine`; ConvertRail cicla 4 tipos.
- Save **v16** (migración no-op desde v15).
- Sprites OpenGFX planos mono/maglev (+82/+164); pendientes/nieve siguen con tinte fallback.
- Tranvía = `RoadType` en OpenTTD → fuera de este corte.

## Fase 7 — NewGRF runtime ✅ (MVP config) / completo pendiente

**MVP (v17):**
- `newgrf_config.rs`: `NewGrfEntry`, stack por defecto OpenGFX, `scan_grf_*` (contenedor v1/v2 + Action 8).
- `GameState.newgrf_stack`; save **v17** + migración desde v16.
- UI Ajustes → «NewGRF…» (lista de solo lectura).

**Pendiente (runtime completo):** Action0–14, callbacks, gfx ≥175, edición/reorden del stack, chunk `NGRF` en `.sav`.

## UI continua

Paridad de ventanas según [`parity/ui_windows_parity.md`](parity/ui_windows_parity.md)
en cada fase (depósito drag, rating, overlay PBS, compañías, NewGRF GUI).
