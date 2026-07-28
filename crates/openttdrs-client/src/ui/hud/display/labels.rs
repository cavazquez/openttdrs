use crate::ui::BuildMenuAction;

#[must_use]
pub(crate) fn tool_hud_label(action: BuildMenuAction) -> &'static str {
    match action {
        BuildMenuAction::Road => "Carretera +",
        BuildMenuAction::RoadX => "Carretera NE-SW",
        BuildMenuAction::RoadY => "Carretera NO-SE",
        BuildMenuAction::Tram => "Tranvía +",
        BuildMenuAction::TramX => "Tranvía NE-SW",
        BuildMenuAction::TramY => "Tranvía NO-SE",
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
        BuildMenuAction::ShipDepot => "Depósito barcos",
        BuildMenuAction::Dock => "Muelle",
        BuildMenuAction::Canal => "Canal",
        BuildMenuAction::River => "Río",
        BuildMenuAction::Buoy => "Boya",
        BuildMenuAction::Aqueduct => "Acueducto",
        BuildMenuAction::Lock => "Esclusa",
        BuildMenuAction::Airport => "Aeropuerto",
        BuildMenuAction::RailBridge => "Puente vía",
        BuildMenuAction::RailTunnel => "Túnel vía",
        BuildMenuAction::RailWaypoint => "Waypoint",
        BuildMenuAction::RoadWaypoint => "Waypoint road",
        BuildMenuAction::RailSignals => {
            "Señales (Ctrl: tipo block/entry/exit/combo/path; Shift+RMB: densidad)"
        }
        BuildMenuAction::RailRemove => "Quitar vía",
        BuildMenuAction::RailConvert => "Convertir vía (al tipo seleccionado)",
        BuildMenuAction::Station => "Parada camión",
        BuildMenuAction::BusStop => "Parada bus",
        BuildMenuAction::Clear => "Demoler (señal: quita sin vía)",
        BuildMenuAction::Orders => "Órdenes",
        BuildMenuAction::BuildHouse => "Casa",
        BuildMenuAction::FoundTown => "Fundar pueblo",
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
        BuildMenuAction::PlantTree => "Plantar árbol",
        BuildMenuAction::PlaceSign => "Cartel",
        BuildMenuAction::BuildLighthouse => "Faro",
        BuildMenuAction::BuildTransmitter => "Transmisor",
        BuildMenuAction::PlaceNewGrfObject => "Objeto",
        BuildMenuAction::JoinStation => "Unir estaciones",
        BuildMenuAction::TramRemove => "Quitar tranvía",
    }
}

#[must_use]
pub(crate) fn tool_hud_hint(action: BuildMenuAction) -> Option<&'static str> {
    match action {
        BuildMenuAction::RoadDepot => Some("comprar vehículo; no carga cargo"),
        BuildMenuAction::RailDepot => Some("comprar tren"),
        BuildMenuAction::ShipDepot => Some("comprar barco; boca hacia agua"),
        BuildMenuAction::Dock => Some("agua costera; carga Goods"),
        BuildMenuAction::Canal => Some("hierba/bosque → agua navegable"),
        BuildMenuAction::River => Some("pintura de río; admite pendiente inclinada"),
        BuildMenuAction::Buoy => Some("agua; waypoint de barcos (sin carga)"),
        BuildMenuAction::Aqueduct => Some("arrastre entre pendientes enfrentadas"),
        BuildMenuAction::Lock => Some("sobre agua; RMB gira eje NS/EW"),
        BuildMenuAction::Airport => Some("picker: tipo/eje; RMB rota; cobertura en ventana"),
        BuildMenuAction::RailSignals => {
            Some("arrastre dens.N; clic sentido; Ctrl+clic tipo; Shift+RMB dens.; RMB dir")
        }
        BuildMenuAction::Station | BuildMenuAction::BusStop => {
            Some("picker NewGRF; hierba junto a vía; carga/descarga")
        }
        BuildMenuAction::PlaceNewGrfObject => Some("picker: vanilla/NewGRF W×H; hierba/bosque"),
        BuildMenuAction::Orders => Some("clic mapa: destino"),
        BuildMenuAction::RailStation => Some("hierba junto a vía"),
        BuildMenuAction::FoundTown => Some("clic en hierba: funda un pueblo nuevo"),
        BuildMenuAction::PlantTree => Some("hierba → bosque; bosque → +1 árbol (máx 4)"),
        BuildMenuAction::PlaceSign => Some("clic: coloca cartel; Mundo → Carteles para lista"),
        BuildMenuAction::BuildLighthouse => Some("1 faro por mapa; hierba/bosque"),
        BuildMenuAction::BuildTransmitter => Some("1 transmisor por mapa; hierba/bosque"),
        BuildMenuAction::Tram | BuildMenuAction::TramX | BuildMenuAction::TramY => {
            Some("overlay m3; vehículos Tram en depósito carretera")
        }
        BuildMenuAction::TramRemove => Some("quita solo el overlay de tranvía"),
        BuildMenuAction::JoinStation => {
            Some("1º clic: conservar; 2º: road adyacente o rail (huella/eje)")
        }
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
