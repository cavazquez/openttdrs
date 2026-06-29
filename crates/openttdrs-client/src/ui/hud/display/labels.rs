use crate::ui::BuildMenuAction;

#[must_use]
pub(crate) fn tool_hud_label(action: BuildMenuAction) -> &'static str {
    match action {
        BuildMenuAction::Road => "Carretera +",
        BuildMenuAction::RoadX => "Carretera NE-SW",
        BuildMenuAction::RoadY => "Carretera NO-SE",
        BuildMenuAction::RoadDepot => "Depósito carretera",
        BuildMenuAction::RoadBridge => "Puente carretera",
        BuildMenuAction::RoadTunnel => "Túnel carretera",
        BuildMenuAction::Rail => "Vía",
        BuildMenuAction::RailX => "Vía NE-SW",
        BuildMenuAction::RailY => "Vía NW-SE",
        BuildMenuAction::RailHorz => "Vía E-O",
        BuildMenuAction::RailVert => "Vía N-S",
        BuildMenuAction::RailStation => "Estación tren",
        BuildMenuAction::RailDepot => "Depósito vía",
        BuildMenuAction::RailBridge => "Puente vía",
        BuildMenuAction::RailTunnel => "Túnel vía",
        BuildMenuAction::RailWaypoint => "Waypoint",
        BuildMenuAction::RailSignals => "Señales (RMB: dirección)",
        BuildMenuAction::RailRemove => "Quitar vía",
        BuildMenuAction::RailConvert => "Convertir vía (pendiente railtypes)",
        BuildMenuAction::Station => "Parada camión",
        BuildMenuAction::BusStop => "Parada bus",
        BuildMenuAction::Clear => "Demoler",
        BuildMenuAction::Orders => "Órdenes",
        BuildMenuAction::BuildHouse => "Casa",
        BuildMenuAction::BuildCoalMine => "Mina carbón",
        BuildMenuAction::BuildIronOreMine => "Mina hierro",
        BuildMenuAction::BuildGoldMine => "Mina oro",
        BuildMenuAction::BuildOilWell => "Pozo petróleo",
        BuildMenuAction::BuildOilRefinery => "Refinería",
        BuildMenuAction::BuildFactory => "Fábrica",
        BuildMenuAction::BuildSawmill => "Aserradero",
        BuildMenuAction::BuildForest => "Bosque",
        BuildMenuAction::BuildFarm => "Granja",
        BuildMenuAction::BuildCottonCandy => "Algodón de azúcar",
        BuildMenuAction::BuildCandyFactory => "Fábrica caramelos",
        BuildMenuAction::BuildBatteryFarm => "Granja baterías",
        BuildMenuAction::BuildColaWells => "Pozo cola",
        BuildMenuAction::BuildToyFactory => "Fábrica juguetes",
        BuildMenuAction::BuildPlasticFountain => "Fuente plástico",
        BuildMenuAction::BuildFizzyDrinkFactory => "Bebidas gaseosas",
        BuildMenuAction::BuildBubbleGenerator => "Generador burbujas",
        BuildMenuAction::BuildToffeeQuarry => "Cantera toffee",
        BuildMenuAction::BuildSugarMine => "Mina azúcar",
        BuildMenuAction::RaiseLand => "Elevar terreno",
        BuildMenuAction::LowerLand => "Bajar terreno",
        BuildMenuAction::LevelLand => "Nivelar terreno",
        BuildMenuAction::BuyLand => "Comprar terreno",
    }
}

#[must_use]
pub(crate) fn tool_hud_hint(action: BuildMenuAction) -> Option<&'static str> {
    match action {
        BuildMenuAction::RoadDepot => Some("comprar vehículo; no carga cargo"),
        BuildMenuAction::RailDepot => Some("comprar tren"),
        BuildMenuAction::RailSignals => Some("clic quita si ya hay; RMB: dirección"),
        BuildMenuAction::Station | BuildMenuAction::BusStop => {
            Some("hierba junto a vía; carga/descarga")
        }
        BuildMenuAction::Orders => Some("clic mapa: destino"),
        BuildMenuAction::RailStation => Some("hierba junto a vía"),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{tool_hud_hint, tool_hud_label};
    use crate::ui::BuildMenuAction;

    #[test]
    fn depot_and_station_labels_differ() {
        assert_ne!(
            tool_hud_label(BuildMenuAction::RoadDepot),
            tool_hud_label(BuildMenuAction::Station)
        );
        assert!(
            tool_hud_hint(BuildMenuAction::RoadDepot)
                .unwrap()
                .contains("no carga")
        );
        assert!(
            tool_hud_hint(BuildMenuAction::Station)
                .unwrap()
                .contains("carga")
        );
    }
}
