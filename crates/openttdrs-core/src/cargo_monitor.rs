//! Monitor efímero de entregas y recogidas de carga.
//!
//! `OpenTTD` mantiene dos mapas (`_cargo_pickups` y `_cargo_deliveries`) que los
//! `GameScripts` consultan entre dos activaciones. La clave empaqueta compañía,
//! cargo y entidad (pueblo o industria) en un `uint32_t`; los contadores son
//! enteros saturantes de 32 bits y una lectura los pone a cero.

use std::collections::BTreeMap;

use crate::cargo::CargoType;
use crate::company::CompanyId;

/// Identificador empaquetado de un monitor de carga (`CargoMonitorID`).
pub type CargoMonitorId = u32;

/// Fuente geográfica de un `CargoPacket`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CargoSource {
    /// Carga producida por una industria del pool nativo.
    Industry(u16),
    /// Carga producida por un pueblo del pool nativo.
    Town(u32),
    /// Fuente desconocida o no representable por el modelo actual.
    Unknown,
}

/// Estado efímero de los monitores de carga activos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CargoMonitor {
    pickups: BTreeMap<CargoMonitorId, i32>,
    deliveries: BTreeMap<CargoMonitorId, i32>,
}

/// Primer bit del número de entidad (pueblo o industria).
pub const MONITOR_ENTITY_START: u8 = 0;
/// Cantidad de bits del número de entidad.
pub const MONITOR_ENTITY_LENGTH: u8 = 16;
/// Bit que distingue industria (`1`) de pueblo (`0`).
pub const MONITOR_INDUSTRY_BIT: u8 = 16;
/// Primer bit del tipo de cargo.
pub const MONITOR_CARGO_START: u8 = 19;
/// Cantidad de bits del tipo de cargo.
pub const MONITOR_CARGO_LENGTH: u8 = 6;
/// Primer bit de la compañía.
pub const MONITOR_COMPANY_START: u8 = 25;
/// Cantidad de bits de la compañía.
pub const MONITOR_COMPANY_LENGTH: u8 = 4;

const MONITOR_ENTITY_MASK: u32 = (1 << MONITOR_ENTITY_LENGTH) - 1;
const MONITOR_CARGO_MASK: u32 = (1 << MONITOR_CARGO_LENGTH) - 1;
const MONITOR_COMPANY_MASK: u32 = (1 << MONITOR_COMPANY_LENGTH) - 1;

/// Codifica un monitor asociado a una industria.
#[must_use]
pub const fn encode_cargo_industry_monitor(
    company: CompanyId,
    cargo: CargoType,
    industry_id: u16,
) -> CargoMonitorId {
    (industry_id as u32 & MONITOR_ENTITY_MASK)
        | (1 << MONITOR_INDUSTRY_BIT)
        | ((cargo.cargo_id() as u32 & MONITOR_CARGO_MASK) << MONITOR_CARGO_START)
        | ((company.0 as u32 & MONITOR_COMPANY_MASK) << MONITOR_COMPANY_START)
}

/// Codifica un monitor asociado a un pueblo.
///
/// Los IDs nativos de `OpenTTD` ocupan 16 bits. El modelo JSON admite `u32`
/// para conservar saves antiguos; al empaquetar se aplica la misma máscara
/// nativa y se evita que bits altos contaminen cargo o compañía.
#[must_use]
pub const fn encode_cargo_town_monitor(
    company: CompanyId,
    cargo: CargoType,
    town_id: u32,
) -> CargoMonitorId {
    (town_id & MONITOR_ENTITY_MASK)
        | ((cargo.cargo_id() as u32 & MONITOR_CARGO_MASK) << MONITOR_CARGO_START)
        | ((company.0 as u32 & MONITOR_COMPANY_MASK) << MONITOR_COMPANY_START)
}

/// Extrae la compañía de un identificador empaquetado.
#[must_use]
pub const fn decode_monitor_company(monitor: CargoMonitorId) -> CompanyId {
    CompanyId(((monitor >> MONITOR_COMPANY_START) & MONITOR_COMPANY_MASK) as u8)
}

/// Extrae el cargo; `None` indica un slot aún no representable por el catálogo.
#[must_use]
pub const fn decode_monitor_cargo(monitor: CargoMonitorId) -> Option<CargoType> {
    CargoType::from_cargo_id(((monitor >> MONITOR_CARGO_START) & MONITOR_CARGO_MASK) as u8)
}

/// Indica si la clave monitoriza una industria.
#[must_use]
pub const fn monitor_monitors_industry(monitor: CargoMonitorId) -> bool {
    monitor & (1 << MONITOR_INDUSTRY_BIT) != 0
}

/// Extrae el ID de industria, si la clave es de industria.
#[must_use]
pub const fn decode_monitor_industry(monitor: CargoMonitorId) -> Option<u16> {
    if !monitor_monitors_industry(monitor) {
        return None;
    }
    Some((monitor & MONITOR_ENTITY_MASK) as u16)
}

/// Extrae el ID de pueblo, si la clave es de pueblo.
#[must_use]
pub const fn decode_monitor_town(monitor: CargoMonitorId) -> Option<u16> {
    if monitor_monitors_industry(monitor) {
        return None;
    }
    Some((monitor & MONITOR_ENTITY_MASK) as u16)
}

impl CargoMonitor {
    /// Registra una entrega final, actualizando pueblo, industria y pickup.
    ///
    /// El mapa de pueblo se actualiza siempre que la estación tenga un pueblo
    /// asociado; el mapa de industria sólo cuando `destination` coincide con
    /// una industria cubierta por esa estación. Los incrementos se ignoran si
    /// el monitor no fue activado previamente, igual que `AddCargoDelivery`.
    pub fn add_cargo_delivery(
        &mut self,
        cargo: CargoType,
        company: CompanyId,
        amount: u32,
        source: CargoSource,
        station_town: Option<u32>,
        destination: Option<u16>,
    ) {
        if amount == 0 {
            return;
        }

        // OpenTTD cuenta la recogida al confirmar la entrega final, no al
        // cargar el vehículo. Sólo un monitor previamente activo acumula.
        let pickup_id = match source {
            CargoSource::Industry(industry_id) => {
                Some(encode_cargo_industry_monitor(company, cargo, industry_id))
            }
            CargoSource::Town(town_id) => Some(encode_cargo_town_monitor(company, cargo, town_id)),
            CargoSource::Unknown => None,
        };
        if let Some(id) = pickup_id {
            add_if_active(&mut self.pickups, id, amount);
        }

        if let Some(town_id) = station_town {
            let id = encode_cargo_town_monitor(company, cargo, town_id);
            add_if_active(&mut self.deliveries, id, amount);
        }
        if let Some(industry_id) = destination {
            let id = encode_cargo_industry_monitor(company, cargo, industry_id);
            add_if_active(&mut self.deliveries, id, amount);
        }
    }

    /// Lee y reinicia el acumulado de entregas de una clave.
    pub fn get_delivery_amount(&mut self, monitor: CargoMonitorId, keep_monitoring: bool) -> i32 {
        get_amount(&mut self.deliveries, monitor, keep_monitoring)
    }

    /// Lee y reinicia el acumulado de recogidas de una clave.
    pub fn get_pickup_amount(&mut self, monitor: CargoMonitorId, keep_monitoring: bool) -> i32 {
        get_amount(&mut self.pickups, monitor, keep_monitoring)
    }

    /// Borra todos los monitores de recogida o sólo los de una compañía.
    pub fn clear_pickup_monitoring(&mut self, company: Option<CompanyId>) {
        clear_for_company(&mut self.pickups, company);
    }

    /// Borra todos los monitores de entrega o sólo los de una compañía.
    pub fn clear_delivery_monitoring(&mut self, company: Option<CompanyId>) {
        clear_for_company(&mut self.deliveries, company);
    }

    /// Devuelve cuántos monitores están activos en cada mapa (diagnóstico).
    #[must_use]
    pub fn active_counts(&self) -> (usize, usize) {
        (self.pickups.len(), self.deliveries.len())
    }
}

fn add_if_active(map: &mut BTreeMap<CargoMonitorId, i32>, id: CargoMonitorId, amount: u32) {
    let Some(value) = map.get_mut(&id) else {
        return;
    };
    let amount = i32::try_from(amount).unwrap_or(i32::MAX);
    *value = value.saturating_add(amount);
}

fn get_amount(
    map: &mut BTreeMap<CargoMonitorId, i32>,
    monitor: CargoMonitorId,
    keep_monitoring: bool,
) -> i32 {
    let Some(value) = map.get_mut(&monitor) else {
        if keep_monitoring {
            map.insert(monitor, 0);
        }
        return 0;
    };
    let result = *value;
    *value = 0;
    if !keep_monitoring {
        map.remove(&monitor);
    }
    result
}

fn clear_for_company(map: &mut BTreeMap<CargoMonitorId, i32>, company: Option<CompanyId>) {
    if let Some(company) = company {
        map.retain(|id, _| decode_monitor_company(*id) != company);
    } else {
        map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_matches_native_bit_layout() {
        let town = encode_cargo_town_monitor(CompanyId(3), CargoType::Coal, 0x1234);
        assert_eq!(town, 0x0608_1234);
        assert_eq!(decode_monitor_company(town), CompanyId(3));
        assert_eq!(decode_monitor_cargo(town), Some(CargoType::Coal));
        assert_eq!(decode_monitor_town(town), Some(0x1234));
        assert!(!monitor_monitors_industry(town));

        let industry = encode_cargo_industry_monitor(CompanyId(3), CargoType::Coal, 0x1234);
        assert_eq!(industry, town | (1 << MONITOR_INDUSTRY_BIT));
        assert!(monitor_monitors_industry(industry));
        assert_eq!(decode_monitor_industry(industry), Some(0x1234));
        assert_eq!(decode_monitor_town(industry), None);
    }

    #[test]
    fn reads_reset_and_activation_is_required() {
        let mut monitor = CargoMonitor::default();
        let town = encode_cargo_town_monitor(CompanyId::PLAYER, CargoType::Passengers, 7);
        let source_town = encode_cargo_town_monitor(CompanyId::PLAYER, CargoType::Passengers, 11);
        let industry = encode_cargo_industry_monitor(CompanyId::PLAYER, CargoType::Coal, 9);
        let _ = monitor.get_pickup_amount(source_town, true);
        monitor.add_cargo_delivery(
            CargoType::Passengers,
            CompanyId::PLAYER,
            4,
            CargoSource::Town(11),
            Some(7),
            None,
        );
        assert_eq!(monitor.get_delivery_amount(town, false), 0);
        assert_eq!(monitor.get_delivery_amount(town, true), 0);

        assert_eq!(monitor.get_pickup_amount(source_town, true), 4);
        assert_eq!(monitor.get_pickup_amount(source_town, true), 0);
        let _ = monitor.get_delivery_amount(industry, true);
        let _ = monitor.get_pickup_amount(industry, true);
        monitor.add_cargo_delivery(
            CargoType::Coal,
            CompanyId::PLAYER,
            u32::MAX,
            CargoSource::Industry(9),
            None,
            Some(9),
        );
        assert_eq!(monitor.get_delivery_amount(industry, true), i32::MAX);
        assert_eq!(monitor.get_pickup_amount(industry, false), i32::MAX);
        assert_eq!(monitor.active_counts(), (1, 2));
        monitor.clear_pickup_monitoring(None);
        monitor.clear_delivery_monitoring(Some(CompanyId::PLAYER));
        assert_eq!(monitor.active_counts(), (0, 0));
    }

    #[test]
    fn industry_delivery_updates_town_and_industry_monitors() {
        let mut monitor = CargoMonitor::default();
        let town = encode_cargo_town_monitor(CompanyId(2), CargoType::Goods, 4);
        let industry = encode_cargo_industry_monitor(CompanyId(2), CargoType::Goods, 8);
        let source = encode_cargo_industry_monitor(CompanyId(2), CargoType::Goods, 3);
        let _ = monitor.get_delivery_amount(town, true);
        let _ = monitor.get_delivery_amount(industry, true);
        let _ = monitor.get_pickup_amount(source, true);

        monitor.add_cargo_delivery(
            CargoType::Goods,
            CompanyId(2),
            12,
            CargoSource::Industry(3),
            Some(4),
            Some(8),
        );
        assert_eq!(monitor.get_delivery_amount(town, true), 12);
        assert_eq!(monitor.get_delivery_amount(industry, true), 12);
        assert_eq!(monitor.get_pickup_amount(source, true), 12);
    }
}
