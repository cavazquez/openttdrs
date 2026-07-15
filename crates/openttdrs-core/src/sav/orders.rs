//! Órdenes de vehículos desde chunks `ORDL` / `ORDR` y resolución a [`VehicleOrder`].

use std::collections::HashMap;

use crate::map::{TileCoord, coord_from_linear_index};
use crate::vehicle::VehicleOrder;

use super::chunks::{RawChunk, find_chunk};
use super::entities::SavStationIndex;
use super::table::{SlRecord, SlValue, record_get};

/// Tipos de orden relevantes (`order_type.h` en `OpenTTD`).
const OT_NOTHING: u8 = 0;
const OT_GOTO_STATION: u8 = 1;
const OT_GOTO_DEPOT: u8 = 2;
const OT_GOTO_WAYPOINT: u8 = 6;
const OT_CONDITIONAL: u8 = 7;
const OTTD_DEPOT_SERVICE: u8 = 1 << 0;
const OTTD_DEPOT_HALT: u8 = 1 << 3;

/// `SLV_105` — listas de órdenes como pool `OrderList` (`ORDL`).
const SLV_105: u16 = 105;
/// `SLV_ORDERS_OWNED_BY_ORDERLIST` — órdenes inline en `ORDL`.
const SLV_ORDERS_OWNED_BY_ORDERLIST: u16 = 354;

/// Orden cruda decodificada del save (`Order::type` + `dest` + `flags`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavOrder {
    pub order_type: u8,
    pub dest: u16,
    /// Byte `Order::flags` (`order_base.h`: unload bits 0–2, load bits 4–6).
    pub flags: u8,
}

/// `OrderUnloadType::NoUnload` en bits 0–2 de `flags`.
const OTTD_UNLOAD_NO_UNLOAD: u8 = 4;
/// `OrderLoadType::FullLoad` / `FullLoadAny` en bits 4–6 de `flags`.
const OTTD_LOAD_FULL: u8 = 2;
const OTTD_LOAD_FULL_ANY: u8 = 3;

#[must_use]
pub(crate) fn stop_flags_from_sav(flags: u8) -> (bool, bool) {
    let unload = flags & 0x07;
    let load = (flags >> 4) & 0x07;
    let no_unload = unload == OTTD_UNLOAD_NO_UNLOAD;
    let full_load = load == OTTD_LOAD_FULL || load == OTTD_LOAD_FULL_ANY;
    (full_load, no_unload)
}

#[derive(Debug, Clone, Copy)]
struct OrdrEntry {
    order_type: u8,
    dest: u16,
    flags: u8,
    /// Índice 1-based del siguiente eslabón (`OldOrderSaveLoadItem::next`).
    next: u32,
}

/// Contexto de importación: listas `ORDL` + pool legacy `ORDR`.
pub(crate) struct SavOrderImport {
    lists: HashMap<u32, Vec<SavOrder>>,
    ordr: HashMap<u32, OrdrEntry>,
    version: u16,
}

impl SavOrderImport {
    #[must_use]
    pub(crate) fn from_chunks(chunks: &[RawChunk], version: u16) -> Self {
        let ordr = ordr_pool_from_chunks(chunks, version);
        let lists = order_lists_from_chunks(chunks, version, &ordr);
        Self {
            lists,
            ordr,
            version,
        }
    }

    /// Resuelve la referencia `common.orders` de un vehículo.
    #[must_use]
    pub(crate) fn orders_for_vehicle_ref(&self, order_ref: u64) -> Vec<SavOrder> {
        if order_ref == 0 {
            return Vec::new();
        }
        if self.version < SLV_105 {
            return chain_from_ordr_ref(u32::try_from(order_ref).unwrap_or(u32::MAX), &self.ordr);
        }
        let list_id = u32::try_from(order_ref.saturating_sub(1)).ok();
        list_id
            .and_then(|id| self.lists.get(&id))
            .cloned()
            .unwrap_or_default()
    }
}

fn table_rows(chunk: &RawChunk, save_version: u16) -> Vec<(u32, SlRecord)> {
    super::array_legacy::chunk_rows(chunk, save_version)
}

fn sav_order_from_fields(order_type: u8, dest: u16, flags: u8) -> Option<SavOrder> {
    if (order_type & 0x0F) == OT_NOTHING {
        None
    } else {
        Some(SavOrder {
            order_type,
            dest,
            flags,
        })
    }
}

fn orders_from_record(record: &SlRecord) -> Vec<SavOrder> {
    let Some(SlValue::Structs(items)) = record_get(record, "orders") else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let order_type = record_get(item, "type")
                .and_then(SlValue::as_u64)
                .and_then(|v| u8::try_from(v).ok())
                .unwrap_or(OT_NOTHING);
            let dest = record_get(item, "dest")
                .and_then(SlValue::as_u64)
                .and_then(|v| u16::try_from(v).ok())
                .unwrap_or(0);
            let flags = record_get(item, "flags")
                .and_then(SlValue::as_u64)
                .and_then(|v| u8::try_from(v).ok())
                .unwrap_or(0);
            sav_order_from_fields(order_type, dest, flags)
        })
        .collect()
}

fn ordr_entry_from_record(record: &SlRecord) -> OrdrEntry {
    OrdrEntry {
        order_type: record_get(record, "type")
            .and_then(SlValue::as_u64)
            .and_then(|v| u8::try_from(v).ok())
            .unwrap_or(OT_NOTHING),
        dest: record_get(record, "dest")
            .and_then(SlValue::as_u64)
            .and_then(|v| u16::try_from(v).ok())
            .unwrap_or(0),
        flags: record_get(record, "flags")
            .and_then(SlValue::as_u64)
            .and_then(|v| u8::try_from(v).ok())
            .unwrap_or(0),
        next: record_get(record, "next")
            .and_then(SlValue::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0),
    }
}

/// Pool legacy `ORDR` (cadena `next` 1-based).
#[must_use]
fn ordr_pool_from_chunks(chunks: &[RawChunk], save_version: u16) -> HashMap<u32, OrdrEntry> {
    let Some(ordr) = find_chunk(chunks, "ORDR") else {
        return HashMap::new();
    };
    table_rows(ordr, save_version)
        .into_iter()
        .map(|(idx, record)| (idx, ordr_entry_from_record(&record)))
        .collect()
}

fn chain_from_ordr_ref(start_ref: u32, pool: &HashMap<u32, OrdrEntry>) -> Vec<SavOrder> {
    if start_ref == 0 {
        return Vec::new();
    }
    let mut ref_idx = start_ref;
    let mut out = Vec::new();
    for _ in 0..512 {
        let key = ref_idx.saturating_sub(1);
        let Some(entry) = pool.get(&key) else {
            break;
        };
        if let Some(order) = sav_order_from_fields(entry.order_type, entry.dest, entry.flags) {
            out.push(order);
        }
        if entry.next == 0 || entry.next == ref_idx {
            break;
        }
        ref_idx = entry.next;
    }
    out
}

fn order_list_from_ordl_record(record: &SlRecord, ordr: &HashMap<u32, OrdrEntry>) -> Vec<SavOrder> {
    let inline = orders_from_record(record);
    if !inline.is_empty() {
        return inline;
    }
    let first = record_get(record, "first")
        .and_then(SlValue::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    chain_from_ordr_ref(first, ordr)
}

/// Listas de órdenes del chunk `ORDL` (índice de pool → secuencia).
#[must_use]
fn order_lists_from_chunks(
    chunks: &[RawChunk],
    version: u16,
    ordr: &HashMap<u32, OrdrEntry>,
) -> HashMap<u32, Vec<SavOrder>> {
    let Some(ordl_chunk) = find_chunk(chunks, "ORDL") else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (idx, record) in table_rows(ordl_chunk, version) {
        let orders = if version >= SLV_ORDERS_OWNED_BY_ORDERLIST {
            orders_from_record(&record)
        } else {
            order_list_from_ordl_record(&record, ordr)
        };
        if !orders.is_empty() {
            out.insert(idx, orders);
        }
    }
    out
}

/// Convierte órdenes del save a destinos jugables (estación/waypoint/depósito/condicional).
#[must_use]
pub(crate) fn vehicle_orders_from_sav(
    sav_orders: &[SavOrder],
    stations: &HashMap<u32, SavStationIndex>,
    map_w: u32,
) -> Vec<VehicleOrder> {
    let mut out = Vec::new();
    for order in sav_orders {
        let ot = order.order_type & 0x0F;
        match ot {
            OT_GOTO_STATION => {
                if let Some(st) = stations.get(&u32::from(order.dest)) {
                    let (full_load, no_unload) = stop_flags_from_sav(order.flags);
                    if st.is_waypoint {
                        out.push(VehicleOrder::waypoint(st.pos));
                    } else {
                        out.push(VehicleOrder::station_with_flags(
                            st.pos, full_load, no_unload,
                        ));
                    }
                }
            }
            OT_GOTO_WAYPOINT => {
                if let Some(st) = stations.get(&u32::from(order.dest)) {
                    out.push(VehicleOrder::waypoint(st.pos));
                }
            }
            OT_GOTO_DEPOT => {
                let pos = coord_from_linear_index(u64::from(order.dest), map_w)
                    .unwrap_or(TileCoord::new(0, 0));
                let halt = order.flags & OTTD_DEPOT_HALT != 0;
                let service = order.flags & OTTD_DEPOT_SERVICE != 0;
                let depot_order = if halt || !service {
                    VehicleOrder::depot(pos)
                } else {
                    VehicleOrder::depot_pass_through(pos)
                };
                out.push(depot_order);
            }
            OT_CONDITIONAL => {
                let comparator = (order.order_type >> 5) & 0x07;
                let value = u8::try_from(order.dest & 0x07FF).unwrap_or(255);
                let jump_to = usize::from(order.flags);
                let condition = match comparator {
                    4 => crate::vehicle::OrderConditionKind::CargoLoadAbove,
                    2 => crate::vehicle::OrderConditionKind::CargoLoadBelow,
                    _ => continue,
                };
                out.push(VehicleOrder::conditional(condition, value, jump_to));
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
mod tests {
    use super::super::chunks::{CH_ARRAY, CH_TABLE, RawChunk};
    use super::*;
    use crate::map::TileCoord;

    fn ordl_header_with_orders_subfields() -> Vec<u8> {
        let mut header = Vec::new();
        header.push(0x1B);
        super::super::table::tests::write_str("orders", &mut header);
        header.push(0);
        header.push(2);
        super::super::table::tests::write_str("type", &mut header);
        header.push(2);
        super::super::table::tests::write_str("flags", &mut header);
        header.push(4);
        super::super::table::tests::write_str("dest", &mut header);
        header.push(2);
        super::super::table::tests::write_str("refit_cargo", &mut header);
        header.push(4);
        super::super::table::tests::write_str("wait_time", &mut header);
        header.push(4);
        super::super::table::tests::write_str("travel_time", &mut header);
        header.push(4);
        super::super::table::tests::write_str("max_speed", &mut header);
        header.push(0);
        header
    }

    fn ordl_body(header: &[u8], rec: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        super::super::table::tests::write_gamma(header.len() as u32 + 1, &mut body);
        body.extend_from_slice(header);
        super::super::table::tests::write_gamma(rec.len() as u32 + 1, &mut body);
        body.extend_from_slice(rec);
        super::super::table::tests::write_gamma(0, &mut body);
        body
    }

    #[test]
    fn parses_ordl_order_list() {
        let mut order = Vec::new();
        order.push(1);
        order.push(0);
        order.extend_from_slice(&2u16.to_be_bytes());
        order.push(0xFF);
        order.extend_from_slice(&0u16.to_be_bytes());
        order.extend_from_slice(&0u16.to_be_bytes());
        order.extend_from_slice(&0u16.to_be_bytes());

        let mut rec = Vec::new();
        rec.push(1);
        rec.extend_from_slice(&order);

        let chunk = RawChunk {
            name: *b"ORDL",
            ch_type: CH_TABLE,
            body: ordl_body(&ordl_header_with_orders_subfields(), &rec),
        };
        let import = SavOrderImport::from_chunks(&[chunk], 360);
        assert_eq!(import.lists.get(&0).map(Vec::len), Some(1));
        assert_eq!(import.lists[&0][0].dest, 2);
    }

    fn gamma_array_body(records: &[&[u8]]) -> Vec<u8> {
        let mut body = Vec::new();
        for r in records {
            super::super::table::tests::write_gamma(r.len() as u32 + 1, &mut body);
            body.extend_from_slice(r);
        }
        super::super::table::tests::write_gamma(0, &mut body);
        body
    }

    #[test]
    fn ordl_first_resolves_ordr_chain() {
        let mut o0 = Vec::new();
        o0.push(1);
        o0.push(0);
        o0.extend_from_slice(&3u16.to_be_bytes());
        o0.extend_from_slice(&0u32.to_be_bytes());

        let mut ordl_rec = Vec::new();
        ordl_rec.extend_from_slice(&1u32.to_be_bytes());

        let ordr = RawChunk {
            name: *b"ORDR",
            ch_type: CH_ARRAY,
            body: gamma_array_body(&[&o0]),
        };
        let order_list = RawChunk {
            name: *b"ORDL",
            ch_type: CH_ARRAY,
            body: gamma_array_body(&[&ordl_rec]),
        };

        let import = SavOrderImport::from_chunks(&[ordr, order_list], 211);
        assert_eq!(import.lists.get(&0).map(Vec::len), Some(1));
        assert_eq!(import.lists[&0][0].dest, 3);
        assert_eq!(import.orders_for_vehicle_ref(1).len(), 1);
    }

    #[test]
    fn maps_station_order_to_tile() {
        let mut stations = HashMap::new();
        stations.insert(
            0,
            SavStationIndex {
                pos: TileCoord::new(5, 2),
                is_waypoint: false,
                facilities: 1,
                name: None,
            },
        );
        let orders = vehicle_orders_from_sav(
            &[SavOrder {
                order_type: OT_GOTO_STATION,
                dest: 0,
                flags: 0,
            }],
            &stations,
            64,
        );
        assert_eq!(orders, vec![VehicleOrder::station(TileCoord::new(5, 2))]);
    }

    #[test]
    fn maps_full_load_and_no_unload_flags() {
        let mut stations = HashMap::new();
        stations.insert(
            1,
            SavStationIndex {
                pos: TileCoord::new(3, 4),
                is_waypoint: false,
                facilities: 1,
                name: None,
            },
        );
        let full = stop_flags_from_sav(OTTD_LOAD_FULL << 4);
        assert!(full.0);
        let no_unload = stop_flags_from_sav(OTTD_UNLOAD_NO_UNLOAD);
        assert!(no_unload.1);
        let orders = vehicle_orders_from_sav(
            &[SavOrder {
                order_type: OT_GOTO_STATION,
                dest: 1,
                flags: (OTTD_LOAD_FULL << 4) | OTTD_UNLOAD_NO_UNLOAD,
            }],
            &stations,
            64,
        );
        assert_eq!(
            orders,
            vec![VehicleOrder::station_with_flags(
                TileCoord::new(3, 4),
                true,
                true
            )]
        );
    }

    #[test]
    fn maps_depot_and_conditional_orders() {
        let orders = vehicle_orders_from_sav(
            &[
                SavOrder {
                    order_type: OT_GOTO_DEPOT,
                    dest: 5 + 2 * 64,
                    flags: OTTD_DEPOT_HALT | (1 << 1),
                },
                SavOrder {
                    order_type: OT_CONDITIONAL | (4 << 5),
                    dest: 50,
                    flags: 2,
                },
            ],
            &HashMap::new(),
            64,
        );
        assert_eq!(orders.len(), 2);
        assert!(matches!(
            orders[0],
            VehicleOrder::Depot {
                depot: TileCoord { x: 5, y: 2 },
                stop: true,
                ..
            }
        ));
        assert_eq!(
            orders[1],
            VehicleOrder::conditional(crate::vehicle::OrderConditionKind::CargoLoadAbove, 50, 2)
        );
    }
}
