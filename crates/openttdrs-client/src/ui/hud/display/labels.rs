use crate::i18n::Locale;
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
        BuildMenuAction::BuildFarmTropic => "Granja tropical",
        BuildMenuAction::BuildCopperOreMine => "Mina de cobre",
        BuildMenuAction::BuildFactoryTropic => "Fábrica tropical",
        BuildMenuAction::BuildFruitPlantation => "Plantación de fruta",
        BuildMenuAction::BuildRubberPlantation => "Plantación de caucho",
        BuildMenuAction::BuildPaperMill => "Papelera",
        BuildMenuAction::BuildFoodProcessingPlant => "Planta de alimentos",
        BuildMenuAction::BuildDiamondMine => "Mina de diamantes",
        BuildMenuAction::BuildWaterSupply => "Suministro de agua",
        BuildMenuAction::BuildLumberMill => "Aserradero tropical",
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

/// Variante localizada de [`tool_hud_label`] para el HUD dinámico.
///
/// Las acciones no son datos de una partida: el HUD las construye a partir de
/// un enum cerrado. Mantener la traducción junto al enum evita intentar
/// traducir por coincidencia parcial nombres de pueblos, vehículos o NewGRF.
#[must_use]
pub(crate) fn localized_tool_hud_label(locale: Locale, action: BuildMenuAction) -> &'static str {
    if locale == Locale::Es {
        return tool_hud_label(action);
    }
    match action {
        BuildMenuAction::Road => "Road +",
        BuildMenuAction::RoadX => "Road NE-SW",
        BuildMenuAction::RoadY => "Road NW-SE",
        BuildMenuAction::Tram => "Tram +",
        BuildMenuAction::TramX => "Tram NE-SW",
        BuildMenuAction::TramY => "Tram NW-SE",
        BuildMenuAction::RoadDepot => "Road depot",
        BuildMenuAction::RoadBridge => "Road bridge",
        BuildMenuAction::RoadTunnel => "Road tunnel",
        BuildMenuAction::Rail => "Rail",
        BuildMenuAction::RailX => "Rail NE-SW",
        BuildMenuAction::RailY => "Rail NW-SE",
        BuildMenuAction::RailHorz => "Rail E-W",
        BuildMenuAction::RailVert => "Rail N-S",
        BuildMenuAction::RailStation => "Train station",
        BuildMenuAction::RailDepot => "Rail depot",
        BuildMenuAction::ShipDepot => "Ship depot",
        BuildMenuAction::Dock => "Dock",
        BuildMenuAction::Canal => "Canal",
        BuildMenuAction::River => "River",
        BuildMenuAction::Buoy => "Buoy",
        BuildMenuAction::Aqueduct => "Aqueduct",
        BuildMenuAction::Lock => "Lock",
        BuildMenuAction::Airport => "Airport",
        BuildMenuAction::RailBridge => "Rail bridge",
        BuildMenuAction::RailTunnel => "Rail tunnel",
        BuildMenuAction::RailWaypoint => "Rail waypoint",
        BuildMenuAction::RoadWaypoint => "Road waypoint",
        BuildMenuAction::RailSignals => {
            "Signals (Ctrl: block/entry/exit/combo/path; Shift+RMB: density)"
        }
        BuildMenuAction::RailRemove => "Remove rail",
        BuildMenuAction::RailConvert => "Convert rail (to selected type)",
        BuildMenuAction::Station => "Truck stop",
        BuildMenuAction::BusStop => "Bus stop",
        BuildMenuAction::Clear => "Demolish (signal: remove without rail)",
        BuildMenuAction::Orders => "Orders",
        BuildMenuAction::BuildHouse => "House",
        BuildMenuAction::FoundTown => "Found town",
        BuildMenuAction::BuildCoalMine => "Coal mine",
        BuildMenuAction::BuildIronOreMine => "Iron ore mine",
        BuildMenuAction::BuildGoldMine => "Gold mine",
        BuildMenuAction::BuildOilWell => "Oil well",
        BuildMenuAction::BuildOilRefinery => "Oil refinery",
        BuildMenuAction::BuildFactory => "Factory",
        BuildMenuAction::BuildSawmill => "Sawmill",
        BuildMenuAction::BuildForest => "Forest",
        BuildMenuAction::BuildFarm => "Farm",
        BuildMenuAction::BuildFarmTropic => "Tropical farm",
        BuildMenuAction::BuildCopperOreMine => "Copper ore mine",
        BuildMenuAction::BuildFactoryTropic => "Tropical factory",
        BuildMenuAction::BuildFruitPlantation => "Fruit plantation",
        BuildMenuAction::BuildRubberPlantation => "Rubber plantation",
        BuildMenuAction::BuildPaperMill => "Paper mill",
        BuildMenuAction::BuildFoodProcessingPlant => "Food processing plant",
        BuildMenuAction::BuildDiamondMine => "Diamond mine",
        BuildMenuAction::BuildWaterSupply => "Water supply",
        BuildMenuAction::BuildLumberMill => "Tropical lumber mill",
        BuildMenuAction::BuildCottonCandy => "Cotton candy forest",
        BuildMenuAction::BuildCandyFactory => "Candy factory",
        BuildMenuAction::BuildBatteryFarm => "Battery farm",
        BuildMenuAction::BuildColaWells => "Cola wells",
        BuildMenuAction::BuildToyFactory => "Toy factory",
        BuildMenuAction::BuildPlasticFountain => "Plastic fountain",
        BuildMenuAction::BuildFizzyDrinkFactory => "Fizzy drink factory",
        BuildMenuAction::BuildBubbleGenerator => "Bubble generator",
        BuildMenuAction::BuildToffeeQuarry => "Toffee quarry",
        BuildMenuAction::BuildSugarMine => "Sugar mine",
        BuildMenuAction::RaiseLand => "Raise land",
        BuildMenuAction::LowerLand => "Lower land",
        BuildMenuAction::LevelLand => "Level land",
        BuildMenuAction::BuyLand => "Buy land",
        BuildMenuAction::PlantTree => "Plant tree",
        BuildMenuAction::PlaceSign => "Sign",
        BuildMenuAction::BuildLighthouse => "Lighthouse",
        BuildMenuAction::BuildTransmitter => "Transmitter",
        BuildMenuAction::PlaceNewGrfObject => "Object",
        BuildMenuAction::JoinStation => "Join stations",
        BuildMenuAction::TramRemove => "Remove tram",
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

/// Variante localizada de [`tool_hud_hint`]. Ver [`localized_tool_hud_label`]
/// para el motivo de no traducir texto libre por sustitución.
#[must_use]
pub(crate) fn localized_tool_hud_hint(
    locale: Locale,
    action: BuildMenuAction,
) -> Option<&'static str> {
    if locale == Locale::Es {
        return tool_hud_hint(action);
    }
    match action {
        BuildMenuAction::RoadDepot => Some("buy vehicle; does not load cargo"),
        BuildMenuAction::RailDepot => Some("buy train"),
        BuildMenuAction::ShipDepot => Some("buy ship; entrance faces water"),
        BuildMenuAction::Dock => Some("coastal water; loads Goods"),
        BuildMenuAction::Canal => Some("grass/forest → navigable water"),
        BuildMenuAction::River => Some("river paint; supports sloped ground"),
        BuildMenuAction::Buoy => Some("water; ship waypoint (no cargo)"),
        BuildMenuAction::Aqueduct => Some("drag between opposing slopes"),
        BuildMenuAction::Lock => Some("on water; RMB rotates N-S/E-W axis"),
        BuildMenuAction::Airport => Some("picker: type/axis; RMB rotates; coverage in window"),
        BuildMenuAction::RailSignals => {
            Some("drag N-S density; click direction; Ctrl+click type; Shift+RMB density; RMB dir")
        }
        BuildMenuAction::Station | BuildMenuAction::BusStop => {
            Some("NewGRF picker; grass beside road; load/unload")
        }
        BuildMenuAction::PlaceNewGrfObject => Some("picker: vanilla/NewGRF W×H; grass/forest"),
        BuildMenuAction::Orders => Some("map click: destination"),
        BuildMenuAction::RailStation => Some("grass beside rail"),
        BuildMenuAction::FoundTown => Some("click grass: found a new town"),
        BuildMenuAction::PlantTree => Some("grass → forest; forest → +1 tree (max 4)"),
        BuildMenuAction::PlaceSign => Some("click: place sign; World → Signs for the list"),
        BuildMenuAction::BuildLighthouse => Some("one lighthouse per map; grass/forest"),
        BuildMenuAction::BuildTransmitter => Some("one transmitter per map; grass/forest"),
        BuildMenuAction::Tram | BuildMenuAction::TramX | BuildMenuAction::TramY => {
            Some("m3 overlay; Tram vehicles at road depot")
        }
        BuildMenuAction::TramRemove => Some("remove only the tram overlay"),
        BuildMenuAction::JoinStation => {
            Some("first click: keep; second: adjacent road or rail (footprint/axis)")
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{localized_tool_hud_hint, localized_tool_hud_label, tool_hud_hint, tool_hud_label};
    use crate::i18n::Locale;
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

    #[test]
    fn english_hud_labels_and_hints_are_not_left_in_spanish() {
        assert_eq!(
            localized_tool_hud_label(Locale::En, BuildMenuAction::RoadDepot),
            "Road depot"
        );
        assert_eq!(
            localized_tool_hud_label(Locale::En, BuildMenuAction::BuildCoalMine),
            "Coal mine"
        );
        assert_eq!(
            localized_tool_hud_hint(Locale::En, BuildMenuAction::Station),
            Some("NewGRF picker; grass beside road; load/unload")
        );
    }
}
