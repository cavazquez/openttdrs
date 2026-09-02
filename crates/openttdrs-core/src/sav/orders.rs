//! Órdenes de vehículos desde chunks `ORDL` / `ORDR` y resolución a [`VehicleOrder`].
//!
//! El wire format encode/decode vive en [`super::orders_codec`].

use std::collections::HashMap;

use crate::cargo::CargoType;

use super::chunks::{RawChunk, find_chunk};
use super::orders_codec::OT_NOTHING;
use super::table::{SlRecord, SlValue, record_get};

pub use super::orders_codec::SavOrder;
pub(crate) use super::orders_codec::vehicle_orders_from_sav;

/// `SLV_105` — listas de órdenes como pool `OrderList` (`ORDL`).
pub(crate) const SLV_105: u16 = 105;
/// `SLV_ORDERS_OWNED_BY_ORDERLIST` — órdenes inline en `ORDL`.
const SLV_ORDERS_OWNED_BY_ORDERLIST: u16 = 354;

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

fn sav_order_from_fields(
    order_type: u8,
    dest: u16,
    flags: u8,
    wait_time: u16,
    travel_time: u16,
    max_speed: u16,
) -> Option<SavOrder> {
    if (order_type & 0x0F) == OT_NOTHING {
        None
    } else {
        Some(SavOrder {
            order_type,
            dest,
            refit_cargo: None,
            flags,
            wait_time,
            travel_time,
            max_speed,
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
            let refit_cargo = record_get(item, "refit_cargo")
                .and_then(SlValue::as_u64)
                .and_then(|v| u8::try_from(v).ok())
                .filter(|id| *id != 0xFF)
                .and_then(CargoType::from_temperate_id);
            let wait_time = record_get(item, "wait_time")
                .and_then(SlValue::as_u64)
                .and_then(|v| u16::try_from(v).ok())
                .unwrap_or(0);
            let travel_time = record_get(item, "travel_time")
                .and_then(SlValue::as_u64)
                .and_then(|v| u16::try_from(v).ok())
                .unwrap_or(0);
            let max_speed = record_get(item, "max_speed")
                .and_then(SlValue::as_u64)
                .and_then(|v| u16::try_from(v).ok())
                .unwrap_or(u16::MAX);
            sav_order_from_fields(order_type, dest, flags, wait_time, travel_time, max_speed).map(
                |mut order| {
                    order.refit_cargo = refit_cargo;
                    order
                },
            )
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
        if let Some(order) =
            sav_order_from_fields(entry.order_type, entry.dest, entry.flags, 0, 0, u16::MAX)
        {
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::cast_possible_truncation)]
mod tests {
    use super::super::chunks::{CH_ARRAY, CH_TABLE, RawChunk};
    use super::super::orders_codec::{
        OT_CONDITIONAL, OT_GOTO_DEPOT, OT_GOTO_STATION, OTTD_DEPOT_HALT, OTTD_LOAD_FULL,
        OTTD_UNLOAD_NO_UNLOAD, stop_flags_from_sav,
    };
    use super::*;
    use crate::map::TileCoord;
    use crate::vehicle::VehicleOrder;

    fn ordl_header_with_orders_subfields() -> Vec<u8> {
        let mut header = Vec::new();
        super::super::orders_codec::append_ordl_orders_header(&mut header).expect("header");
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
        order.push(CargoType::Coal.temperate_id());
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
        assert_eq!(import.lists[&0][0].refit_cargo, Some(CargoType::Coal));
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
            super::super::entities::SavStationIndex {
                pos: TileCoord::new(5, 2),
                is_waypoint: false,
                facilities: 1,
                name: None,
                string_id: None,
                town_id: None,
                airport_type: 0,
                airport_w: 0,
                airport_h: 0,
                airport_layout: 0,
                airport_rotation: 0,
                airport_blocks: 0,
                airport_persistent_storage_id: None,
            },
        );
        // Middle = bits 4–5 = 1 (default `VehicleOrder::station`).
        let order_type = OT_GOTO_STATION | (1 << 4);
        let orders = vehicle_orders_from_sav(
            &[SavOrder {
                order_type,
                dest: 0,
                refit_cargo: None,
                flags: 0,
                wait_time: 0,
                travel_time: 0,
                max_speed: u16::MAX,
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
            super::super::entities::SavStationIndex {
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
                airport_rotation: 0,
                airport_blocks: 0,
                airport_persistent_storage_id: None,
            },
        );
        let full = stop_flags_from_sav(OTTD_LOAD_FULL << 4);
        assert!(full.0);
        let no_unload = stop_flags_from_sav(OTTD_UNLOAD_NO_UNLOAD);
        assert!(no_unload.1);
        let order_type = OT_GOTO_STATION | (1 << 4); // Middle
        let orders = vehicle_orders_from_sav(
            &[SavOrder {
                order_type,
                dest: 1,
                refit_cargo: None,
                flags: (OTTD_LOAD_FULL << 4) | OTTD_UNLOAD_NO_UNLOAD,
                wait_time: 0,
                travel_time: 0,
                max_speed: u16::MAX,
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
                    refit_cargo: None,
                    flags: OTTD_DEPOT_HALT | (1 << 1),
                    wait_time: 0,
                    travel_time: 0,
                    max_speed: u16::MAX,
                },
                SavOrder {
                    order_type: OT_CONDITIONAL | (4 << 5),
                    dest: 50,
                    refit_cargo: None,
                    flags: 2,
                    wait_time: 0,
                    travel_time: 0,
                    max_speed: u16::MAX,
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
