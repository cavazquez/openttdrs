//! Codec ORDL simétrico: constantes, flags, wire format y header (#149).
//!
//! No maneja `current_order` ni resolución de pools `ORDL`/`ORDR`
//! (eso vive en [`super::orders`]).

use std::collections::HashMap;

use crate::cargo::CargoType;
use crate::map::{TileCoord, coord_from_linear_index, coord_to_linear_index};
use crate::vehicle::{
    OrderConditionKind, OrderLoadType, OrderNonStop, OrderStopLocation, OrderUnloadType,
    VehicleOrder,
};

use super::SavError;
use super::entities::SavStationIndex;
use super::write::codec::write_str;

/// Tipos de orden relevantes (`order_type.h` en `OpenTTD`).
pub(crate) const OT_NOTHING: u8 = 0;
pub(crate) const OT_GOTO_STATION: u8 = 1;
pub(crate) const OT_GOTO_DEPOT: u8 = 2;
/// Orden implícita insertada al visitar estación (`OT_IMPLICIT`).
pub(crate) const OT_IMPLICIT: u8 = 4;
pub(crate) const OT_GOTO_WAYPOINT: u8 = 6;
pub(crate) const OT_CONDITIONAL: u8 = 7;

pub(crate) const OTTD_DEPOT_SERVICE: u8 = 1 << 0;
pub(crate) const OTTD_DEPOT_PART_OF_ORDERS: u8 = 1 << 1;
pub(crate) const OTTD_DEPOT_HALT: u8 = 1 << 3;

/// `OrderUnloadType::NoUnload` en bits 0–2 de `flags`.
pub(crate) const OTTD_UNLOAD_NO_UNLOAD: u8 = 4;
/// `OrderUnloadType::Unload` en bits 0–2.
pub(crate) const OTTD_UNLOAD: u8 = 1;
/// `OrderUnloadType::Transfer` en bits 0–2.
pub(crate) const OTTD_UNLOAD_TRANSFER: u8 = 2;
/// `OrderLoadType::FullLoad` / `FullLoadAny` en bits 4–6 de `flags`.
pub(crate) const OTTD_LOAD_FULL: u8 = 2;
pub(crate) const OTTD_LOAD_FULL_ANY: u8 = 3;
/// `OrderLoadType::NoLoad` en bits 4–6.
pub(crate) const OTTD_LOAD_NO_LOAD: u8 = 4;
/// `OrderNonStopFlag::GoVia` en bit 6 de `type`.
pub(crate) const OTTD_NON_STOP_GO_VIA: u8 = 1 << 6;
/// `OrderStopLocation` en bits 4–5 de `type` (`order_base.h` Get/SetStopLocation).
pub(crate) const OTTD_STOP_LOCATION_SHIFT: u8 = 4;
pub(crate) const OTTD_STOP_LOCATION_MASK: u8 = 0x3 << OTTD_STOP_LOCATION_SHIFT;

/// Bytes por orden en el wire ORDL moderno:
/// `type(1) | flags(1) | dest(2) | refit(1) | wait(2) | travel(2) | max_speed(2)`.
pub(crate) const ORDER_WIRE_LEN: usize = 11;

/// Orden cruda decodificada del save (`Order::type` + `dest` + `flags` + horario).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavOrder {
    pub order_type: u8,
    pub dest: u16,
    /// Byte `Order::flags` (`order_base.h`: unload bits 0–2, load bits 4–6).
    pub flags: u8,
    /// `Order::wait_time` (ticks).
    pub wait_time: u16,
    /// `Order::travel_time` (ticks).
    pub travel_time: u16,
}

/// Flags de parada de estación decodificados del wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationOrderFlags {
    pub load_type: OrderLoadType,
    pub unload_type: OrderUnloadType,
    pub non_stop: OrderNonStop,
    pub stop_location: OrderStopLocation,
}

#[must_use]
pub(crate) fn station_flags_from_sav(order_type: u8, flags: u8) -> StationOrderFlags {
    let unload = flags & 0x07;
    let load = (flags >> 4) & 0x07;
    let non_stop = if order_type & OTTD_NON_STOP_GO_VIA != 0 {
        OrderNonStop::StopAtIntermediate
    } else {
        OrderNonStop::NonStopDestination
    };
    let stop_location = OrderStopLocation::from_u8(
        (order_type & OTTD_STOP_LOCATION_MASK) >> OTTD_STOP_LOCATION_SHIFT,
    );
    StationOrderFlags {
        load_type: match load {
            OTTD_LOAD_FULL => OrderLoadType::FullLoad,
            OTTD_LOAD_FULL_ANY => OrderLoadType::FullLoadAny,
            OTTD_LOAD_NO_LOAD => OrderLoadType::NoLoad,
            _ => OrderLoadType::LoadIfPossible,
        },
        unload_type: match unload {
            OTTD_UNLOAD => OrderUnloadType::Unload,
            OTTD_UNLOAD_TRANSFER => OrderUnloadType::Transfer,
            OTTD_UNLOAD_NO_UNLOAD => OrderUnloadType::NoUnload,
            _ => OrderUnloadType::UnloadIfPossible,
        },
        non_stop,
        stop_location,
    }
}

#[must_use]
#[allow(dead_code)]
pub(crate) fn stop_flags_from_sav(flags: u8) -> (bool, bool) {
    let parsed = station_flags_from_sav(OT_GOTO_STATION, flags);
    (
        matches!(
            parsed.load_type,
            OrderLoadType::FullLoad | OrderLoadType::FullLoadAny
        ),
        parsed.unload_type == OrderUnloadType::NoUnload,
    )
}

#[must_use]
pub(crate) fn station_flags_to_sav(order: &VehicleOrder) -> (u8, u8) {
    let VehicleOrder::Station {
        load_type,
        unload_type,
        non_stop,
        stop_location,
        ..
    } = *order
    else {
        return (OT_GOTO_STATION, 0);
    };
    let mut order_type = OT_GOTO_STATION;
    if non_stop == OrderNonStop::StopAtIntermediate {
        order_type |= OTTD_NON_STOP_GO_VIA;
    }
    order_type |= (stop_location.as_u8() << OTTD_STOP_LOCATION_SHIFT) & OTTD_STOP_LOCATION_MASK;
    let mut flags = 0u8;
    flags |= match unload_type {
        OrderUnloadType::UnloadIfPossible => 0,
        OrderUnloadType::Unload => OTTD_UNLOAD,
        OrderUnloadType::Transfer => OTTD_UNLOAD_TRANSFER,
        OrderUnloadType::NoUnload => OTTD_UNLOAD_NO_UNLOAD,
    };
    flags |= match load_type {
        OrderLoadType::LoadIfPossible => 0,
        OrderLoadType::FullLoad => OTTD_LOAD_FULL << 4,
        OrderLoadType::FullLoadAny => OTTD_LOAD_FULL_ANY << 4,
        OrderLoadType::NoLoad => OTTD_LOAD_NO_LOAD << 4,
    };
    (order_type, flags)
}

#[must_use]
#[allow(dead_code)]
pub(crate) fn stop_flags_to_sav(full_load: bool, no_unload: bool) -> u8 {
    let mut flags = 0u8;
    if full_load {
        flags |= OTTD_LOAD_FULL << 4;
    }
    if no_unload {
        flags |= OTTD_UNLOAD_NO_UNLOAD;
    }
    flags
}

#[must_use]
pub(crate) fn depot_flags_to_sav(stop: bool) -> u8 {
    let mut flags = OTTD_DEPOT_PART_OF_ORDERS;
    if stop {
        flags |= OTTD_DEPOT_HALT;
    } else {
        flags |= OTTD_DEPOT_SERVICE;
    }
    flags
}

/// `true` = detener en depósito (halt o sin service).
#[must_use]
pub(crate) fn depot_stop_from_sav(flags: u8) -> bool {
    let halt = flags & OTTD_DEPOT_HALT != 0;
    let service = flags & OTTD_DEPOT_SERVICE != 0;
    halt || !service
}

/// Empaqueta campos en el layout ORDL de 11 bytes.
#[must_use]
pub(crate) fn encode_order_wire(
    order_type: u8,
    flags: u8,
    dest: u16,
    refit: u8,
    wait_time: u16,
    travel_time: u16,
) -> [u8; ORDER_WIRE_LEN] {
    let mut out = [0u8; ORDER_WIRE_LEN];
    out[0] = order_type;
    out[1] = flags;
    out[2..4].copy_from_slice(&dest.to_be_bytes());
    out[4] = refit;
    out[5..7].copy_from_slice(&wait_time.to_be_bytes());
    out[7..9].copy_from_slice(&travel_time.to_be_bytes());
    // max_speed queda en cero (no exportado).
    out
}

/// Desempaqueta `type`/`flags`/`dest`/`refit`/horario del wire.
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn decode_order_wire(bytes: &[u8]) -> Option<(SavOrder, u8)> {
    if bytes.len() < ORDER_WIRE_LEN {
        return None;
    }
    let wait_time = u16::from_be_bytes([bytes[5], bytes[6]]);
    let travel_time = u16::from_be_bytes([bytes[7], bytes[8]]);
    Some((
        SavOrder {
            order_type: bytes[0],
            flags: bytes[1],
            dest: u16::from_be_bytes([bytes[2], bytes[3]]),
            wait_time,
            travel_time,
        },
        bytes[4],
    ))
}

/// Codifica una [`VehicleOrder`] al wire ORDL.
///
/// `station_id` resuelve el índice STNN de una tesela de estación/waypoint.
#[must_use]
pub(crate) fn encode_vehicle_order(
    order: &VehicleOrder,
    station_id: impl Fn(TileCoord) -> Option<u16>,
    map_w: u32,
) -> Option<[u8; ORDER_WIRE_LEN]> {
    let (order_type, dest, flags, refit, wait_time, travel_time) = match *order {
        VehicleOrder::Station {
            station,
            wait_ticks,
            travel_ticks,
            implicit,
            ..
        } => {
            let id = station_id(station)?;
            if implicit {
                (
                    OT_IMPLICIT,
                    id,
                    0,
                    0xFFu8,
                    u16::try_from(wait_ticks).unwrap_or(u16::MAX),
                    u16::try_from(travel_ticks).unwrap_or(u16::MAX),
                )
            } else {
                let (order_type, flags) = station_flags_to_sav(order);
                (
                    order_type,
                    id,
                    flags,
                    0xFFu8,
                    u16::try_from(wait_ticks).unwrap_or(u16::MAX),
                    u16::try_from(travel_ticks).unwrap_or(u16::MAX),
                )
            }
        }
        VehicleOrder::Waypoint {
            waypoint,
            travel_ticks,
        } => {
            let id = station_id(waypoint)?;
            (
                OT_GOTO_WAYPOINT,
                id,
                0,
                0xFF,
                0,
                u16::try_from(travel_ticks).unwrap_or(u16::MAX),
            )
        }
        VehicleOrder::Depot {
            depot,
            stop,
            refit_cargo,
            wait_ticks,
            travel_ticks,
        } => {
            let id = u16::try_from(coord_to_linear_index(depot, map_w)?).ok()?;
            let refit = refit_cargo.map_or(0xFF, CargoType::temperate_id);
            (
                OT_GOTO_DEPOT,
                id,
                depot_flags_to_sav(stop),
                refit,
                u16::try_from(wait_ticks).unwrap_or(u16::MAX),
                u16::try_from(travel_ticks).unwrap_or(u16::MAX),
            )
        }
        VehicleOrder::Conditional {
            condition,
            value,
            jump_to,
        } => {
            let comparator: u8 = match condition {
                OrderConditionKind::CargoLoadAbove => 4,
                OrderConditionKind::CargoLoadBelow => 2,
            };
            let order_type = OT_CONDITIONAL | (comparator << 5);
            let flags = u8::try_from(jump_to.min(255)).unwrap_or(255);
            let dest = u16::from(value);
            (order_type, dest, flags, 0xFF, 0, 0)
        }
        VehicleOrder::Tile(_) => return None,
    };
    Some(encode_order_wire(
        order_type,
        flags,
        dest,
        refit,
        wait_time,
        travel_time,
    ))
}

/// Convierte órdenes del save a destinos jugables (estación/waypoint/depósito/condicional).
#[must_use]
pub(crate) fn vehicle_orders_from_sav(
    sav_orders: &[SavOrder],
    stations: &HashMap<u32, SavStationIndex>,
    map_w: u32,
) -> Vec<VehicleOrder> {
    let station_for_dest = |dest: u16| {
        stations.get(&u32::from(dest)).or_else(|| {
            // Algunos saves modernos con una sola estación conservan el destino
            // como 0 aunque el pool `STNN` empieza en 1. No adivinar cuando hay
            // más de una estación: ahí el ID exacto sigue siendo obligatorio.
            (stations.len() == 1)
                .then(|| stations.values().next())
                .flatten()
        })
    };
    let mut out = Vec::new();
    for order in sav_orders {
        let ot = order.order_type & 0x0F;
        match ot {
            OT_GOTO_STATION | OT_IMPLICIT => {
                if let Some(st) = station_for_dest(order.dest) {
                    let parsed = station_flags_from_sav(order.order_type, order.flags);
                    if st.is_waypoint {
                        out.push(VehicleOrder::Waypoint {
                            waypoint: st.pos,
                            travel_ticks: u32::from(order.travel_time),
                        });
                    } else if ot == OT_IMPLICIT {
                        out.push(VehicleOrder::implicit(st.pos));
                    } else {
                        out.push(VehicleOrder::Station {
                            station: st.pos,
                            load_type: parsed.load_type,
                            unload_type: parsed.unload_type,
                            non_stop: parsed.non_stop,
                            stop_location: parsed.stop_location,
                            wait_ticks: u32::from(order.wait_time),
                            travel_ticks: u32::from(order.travel_time),
                            implicit: false,
                        });
                    }
                }
            }
            OT_GOTO_WAYPOINT => {
                if let Some(st) = station_for_dest(order.dest) {
                    out.push(VehicleOrder::Waypoint {
                        waypoint: st.pos,
                        travel_ticks: u32::from(order.travel_time),
                    });
                }
            }
            OT_GOTO_DEPOT => {
                let pos = coord_from_linear_index(u64::from(order.dest), map_w)
                    .unwrap_or(TileCoord::new(0, 0));
                let depot_order = if depot_stop_from_sav(order.flags) {
                    VehicleOrder::depot(pos)
                } else {
                    VehicleOrder::depot_pass_through(pos)
                };
                out.push(
                    depot_order
                        .with_wait_ticks(u32::from(order.wait_time))
                        .unwrap_or(depot_order)
                        .with_travel_ticks(u32::from(order.travel_time)),
                );
            }
            OT_CONDITIONAL => {
                let comparator = (order.order_type >> 5) & 0x07;
                let value = u8::try_from(order.dest & 0x07FF).unwrap_or(255);
                let jump_to = usize::from(order.flags);
                let condition = match comparator {
                    4 => OrderConditionKind::CargoLoadAbove,
                    2 => OrderConditionKind::CargoLoadBelow,
                    _ => continue,
                };
                out.push(VehicleOrder::conditional(condition, value, jump_to));
            }
            _ => {}
        }
    }
    out
}

/// Header SL del struct anidado `orders` del chunk ORDL.
///
/// # Errors
///
/// Falla si algún string del header no cabe en gamma.
pub(crate) fn append_ordl_orders_header(header: &mut Vec<u8>) -> Result<(), SavError> {
    header.push(0x1B); // STRUCT | HAS_LENGTH
    write_str("orders", header)?;
    header.push(0); // fin lista top-level → subcampos de orders
    header.push(2);
    write_str("type", header)?;
    header.push(2);
    write_str("flags", header)?;
    header.push(4);
    write_str("dest", header)?;
    header.push(2);
    write_str("refit_cargo", header)?;
    header.push(4);
    write_str("wait_time", header)?;
    header.push(4);
    write_str("travel_time", header)?;
    header.push(4);
    write_str("max_speed", header)?;
    header.push(0);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;

    #[test]
    fn stop_flags_roundtrip() {
        assert_eq!(
            stop_flags_from_sav(stop_flags_to_sav(false, false)),
            (false, false)
        );
        assert_eq!(
            stop_flags_from_sav(stop_flags_to_sav(true, false)),
            (true, false)
        );
        assert_eq!(
            stop_flags_from_sav(stop_flags_to_sav(false, true)),
            (false, true)
        );
        assert_eq!(
            stop_flags_from_sav(stop_flags_to_sav(true, true)),
            (true, true)
        );
        // El helper booleano considera ambos modos como carga completa.
        assert!(stop_flags_from_sav(OTTD_LOAD_FULL_ANY << 4).0);
    }

    #[test]
    fn station_flags_preserve_transfer_no_load_and_timetable() {
        use crate::vehicle::OrderNonStop;
        let flags = station_flags_from_sav(
            OT_GOTO_STATION | OTTD_NON_STOP_GO_VIA,
            (OTTD_LOAD_NO_LOAD << 4) | OTTD_UNLOAD_TRANSFER,
        );
        assert_eq!(flags.load_type, OrderLoadType::NoLoad);
        assert_eq!(flags.unload_type, OrderUnloadType::Transfer);
        assert_eq!(flags.non_stop, OrderNonStop::StopAtIntermediate);

        let order = VehicleOrder::station_with_load_unload_flags(
            TileCoord::new(2, 2),
            false,
            false,
            true,
            false,
            true,
            OrderNonStop::StopAtIntermediate,
        )
        .with_wait_ticks(120)
        .unwrap()
        .with_travel_ticks(240);
        let wire = encode_vehicle_order(&order, |_| Some(3), 64).unwrap();
        let (sav, _) = decode_order_wire(&wire).unwrap();
        assert_eq!(sav.wait_time, 120);
        assert_eq!(sav.travel_time, 240);
        let mut stations = HashMap::new();
        stations.insert(
            3,
            SavStationIndex {
                pos: TileCoord::new(2, 2),
                is_waypoint: false,
                facilities: 1,
                name: None,
                string_id: None,
                town_id: None,
                airport_type: 0,
                airport_w: 0,
                airport_h: 0,
                airport_layout: 0,
                airport_blocks: 0,
            },
        );
        let decoded = vehicle_orders_from_sav(&[sav], &stations, 64);
        assert_eq!(decoded, vec![order]);
    }

    #[test]
    fn full_load_any_flag_is_not_collapsed_to_full_load() {
        let flags = station_flags_from_sav(OT_GOTO_STATION, OTTD_LOAD_FULL_ANY << 4);
        assert_eq!(flags.load_type, OrderLoadType::FullLoadAny);
    }

    #[test]
    fn all_station_load_unload_types_roundtrip_ordl_flags() {
        let load_types = [
            OrderLoadType::LoadIfPossible,
            OrderLoadType::FullLoad,
            OrderLoadType::FullLoadAny,
            OrderLoadType::NoLoad,
        ];
        let unload_types = [
            OrderUnloadType::UnloadIfPossible,
            OrderUnloadType::Unload,
            OrderUnloadType::Transfer,
            OrderUnloadType::NoUnload,
        ];

        for load_type in load_types {
            for unload_type in unload_types {
                let order = VehicleOrder::station_with_types(
                    TileCoord::new(2, 2),
                    load_type,
                    unload_type,
                    OrderNonStop::NonStopDestination,
                );
                let wire = encode_vehicle_order(&order, |_| Some(7), 64).unwrap();
                let (sav, _) = decode_order_wire(&wire).unwrap();
                let decoded = station_flags_from_sav(sav.order_type, sav.flags);
                assert_eq!(decoded.load_type, load_type);
                assert_eq!(decoded.unload_type, unload_type);
            }
        }
    }

    #[test]
    fn depot_flags_roundtrip() {
        assert!(depot_stop_from_sav(depot_flags_to_sav(true)));
        assert!(!depot_stop_from_sav(depot_flags_to_sav(false)));
    }

    #[test]
    fn station_wire_roundtrip_preserves_flags() {
        let order = VehicleOrder::station_with_flags(TileCoord::new(3, 4), true, true);
        let wire = encode_vehicle_order(&order, |_| Some(1), 64).unwrap();
        let (sav, refit) = decode_order_wire(&wire).unwrap();
        assert_eq!(sav.order_type & 0x0F, OT_GOTO_STATION);
        assert_eq!(sav.dest, 1);
        assert_eq!(refit, 0xFF);
        let mut stations = HashMap::new();
        stations.insert(
            1,
            SavStationIndex {
                pos: TileCoord::new(3, 4),
                is_waypoint: false,
                facilities: 1,
                name: None,
                string_id: None,
                town_id: None,
                airport_type: 0,
                airport_w: 0,
                airport_h: 0,
                airport_layout: 0,
                airport_blocks: 0,
            },
        );
        let decoded = vehicle_orders_from_sav(&[sav], &stations, 64);
        assert_eq!(decoded, vec![order]);
    }

    #[test]
    fn station_order_falls_back_to_only_station_for_modern_pool_offset() {
        let mut stations = HashMap::new();
        stations.insert(
            1,
            SavStationIndex {
                pos: TileCoord::new(7, 9),
                is_waypoint: false,
                facilities: 1,
                name: None,
                string_id: None,
                town_id: None,
                airport_type: 0,
                airport_w: 0,
                airport_h: 0,
                airport_layout: 0,
                airport_blocks: 0,
            },
        );
        let order = SavOrder {
            // Middle (default de `VehicleOrder::station`) en bits 4–5.
            order_type: OT_GOTO_STATION | (1 << 4),
            dest: 0,
            flags: 0,
            wait_time: 0,
            travel_time: 0,
        };
        assert_eq!(
            vehicle_orders_from_sav(&[order], &stations, 64),
            vec![VehicleOrder::station(TileCoord::new(7, 9))]
        );
    }

    #[test]
    fn depot_and_conditional_wire_roundtrip() {
        let depot = VehicleOrder::depot(TileCoord::new(5, 2));
        let cond = VehicleOrder::conditional(OrderConditionKind::CargoLoadAbove, 50, 2);
        let map_w = 64u32;
        let depot_wire = encode_vehicle_order(&depot, |_| None, map_w).unwrap();
        let cond_wire = encode_vehicle_order(&cond, |_| None, map_w).unwrap();
        let (depot_sav, _) = decode_order_wire(&depot_wire).unwrap();
        let (cond_sav, _) = decode_order_wire(&cond_wire).unwrap();
        let decoded = vehicle_orders_from_sav(&[depot_sav, cond_sav], &HashMap::new(), map_w);
        assert_eq!(decoded, vec![depot, cond]);
    }

    #[test]
    fn waypoint_wire_roundtrip() {
        let order = VehicleOrder::waypoint(TileCoord::new(1, 1));
        let wire = encode_vehicle_order(&order, |_| Some(7), 32).unwrap();
        let (sav, _) = decode_order_wire(&wire).unwrap();
        assert_eq!(sav.order_type & 0x0F, OT_GOTO_WAYPOINT);
        let mut stations = HashMap::new();
        stations.insert(
            7,
            SavStationIndex {
                pos: TileCoord::new(1, 1),
                is_waypoint: true,
                facilities: 0,
                name: None,
                string_id: None,
                town_id: None,
                airport_type: 0,
                airport_w: 0,
                airport_h: 0,
                airport_layout: 0,
                airport_blocks: 0,
            },
        );
        assert_eq!(vehicle_orders_from_sav(&[sav], &stations, 32), vec![order]);
    }

    #[test]
    fn ordl_header_is_stable() {
        let mut header = Vec::new();
        append_ordl_orders_header(&mut header).unwrap();
        assert_eq!(header.first().copied(), Some(0x1B));
        assert!(header.windows(6).any(|w| w == b"orders"));
        assert!(header.windows(4).any(|w| w == b"type"));
        assert!(header.windows(4).any(|w| w == b"dest"));
        assert_eq!(*header.last().unwrap(), 0);
        assert_eq!(
            header,
            hex_literal_ordl_header(),
            "header ORDL debe coincidir byte-a-byte con el export histórico"
        );
    }

    /// Golden del header ORDL previo al codec (#149).
    fn hex_literal_ordl_header() -> Vec<u8> {
        // 1b 06 "orders" 00 02 04 "type" 02 05 "flags" 04 04 "dest" 02 0b "refit_cargo"
        // 04 09 "wait_time" 04 0b "travel_time" 04 09 "max_speed" 00
        vec![
            0x1b, 0x06, b'o', b'r', b'd', b'e', b'r', b's', 0x00, 0x02, 0x04, b't', b'y', b'p',
            b'e', 0x02, 0x05, b'f', b'l', b'a', b'g', b's', 0x04, 0x04, b'd', b'e', b's', b't',
            0x02, 0x0b, b'r', b'e', b'f', b'i', b't', b'_', b'c', b'a', b'r', b'g', b'o', 0x04,
            0x09, b'w', b'a', b'i', b't', b'_', b't', b'i', b'm', b'e', 0x04, 0x0b, b't', b'r',
            b'a', b'v', b'e', b'l', b'_', b't', b'i', b'm', b'e', 0x04, 0x09, b'm', b'a', b'x',
            b'_', b's', b'p', b'e', b'e', b'd', 0x00,
        ]
    }
}
