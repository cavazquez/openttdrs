//! Funciones de formato para noticias: dinero, nombres de carga, etiquetas.

use crate::cargo::CargoType;
use crate::vehicle::VehicleKind;

use super::queue::{NewsDisplayMode, NewsType};

#[must_use]
pub fn cargo_display_name(cargo: CargoType) -> &'static str {
    cargo.display_name()
}

#[must_use]
pub fn format_money(amount: i64) -> String {
    let sign = if amount < 0 { "-" } else { "" };
    let abs = amount.unsigned_abs();
    if abs >= 1_000_000 {
        let whole = abs / 1_000_000;
        let frac = (abs % 1_000_000) / 100_000;
        format!("{sign}${whole}.{frac}M")
    } else if abs >= 10_000 {
        let whole = abs / 1_000;
        let frac = (abs % 1_000) / 100;
        format!("{sign}${whole}.{frac}K")
    } else {
        format!("{sign}${abs}")
    }
}

#[must_use]
pub fn news_type_label(news_type: NewsType) -> &'static str {
    match news_type {
        NewsType::CargoDelivered => "Entrega de carga",
        NewsType::FirstCargoDelivered => "Primera entrega",
        NewsType::FirstVehicleRunning => "Primer vehículo en marcha",
        NewsType::VehicleAdvice => "Avisos de vehículo",
        NewsType::Accident => "Accidentes",
        NewsType::CompanyInfo => "Compañías",
        NewsType::IndustryClose => "Cierre de industria",
        NewsType::Economy => "Economía",
    }
}

#[must_use]
pub fn news_display_mode_label(mode: NewsDisplayMode) -> &'static str {
    match mode {
        NewsDisplayMode::Off => "Off",
        NewsDisplayMode::Summary => "Summary",
        NewsDisplayMode::Full => "Full",
    }
}

#[must_use]
pub fn vehicle_kind_label(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::Bus => "autobús",
        VehicleKind::Truck => "camión",
        VehicleKind::Tram => "tranvía",
        VehicleKind::Train => "tren",
        VehicleKind::Ship => "barco",
        VehicleKind::Aircraft => "avión",
    }
}
