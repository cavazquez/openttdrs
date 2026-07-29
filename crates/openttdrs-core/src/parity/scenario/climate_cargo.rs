//! Cadenas de cargo por clima (#224): producción → procesamiento → pago.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::Climate;
use crate::cargo::CargoType;
use crate::economy::{transported_goods_income, transported_goods_income_for_climate};
use crate::industry::{Industry, IndustryKind, IndustrySpec};
use crate::map::TileCoord;
use crate::station::{Station, StopKind};

fn chain_produce_process_pay(
    extract: IndustrySpec,
    processor: IndustrySpec,
    input_fill: &[(CargoType, u32)],
    deliver_cargo: CargoType,
    deliver_amount: u32,
    distance: u32,
    transit_days: u16,
) -> (u32, u32, i64) {
    let extract_pos = TileCoord::new(0, 0);
    let mut extractor =
        Industry::with_tiles_spec(extract_pos, extract.kind(), extract, vec![extract_pos], 0);
    extractor.produce(256);
    let produced = extractor.stock;

    let proc_pos = TileCoord::new(4, 4);
    let mut processor_ind =
        Industry::with_tiles_spec(proc_pos, processor.kind(), processor, vec![proc_pos], 0);
    let mut stations = vec![Station::new_with_kind(
        TileCoord::new(5, 4),
        StopKind::TruckStop,
    )];
    for &(cargo, amount) in input_fill {
        stations[0].cargo_stock.add(cargo, amount);
    }
    assert!(
        processor_ind.produce_from_nearby_stations(&mut stations, 512),
        "processor {processor:?} should consume inputs"
    );
    let processed = processor_ind.stock;
    assert_eq!(processor_ind.output_cargo(), deliver_cargo);

    let pay = transported_goods_income(
        deliver_amount,
        distance,
        transit_days,
        deliver_cargo,
        1 << 16,
    );
    (produced, processed, pay)
}

#[test]
fn temperate_catalog_matches_openttd() {
    let cargos = CargoType::for_climate(Climate::Temperate);
    assert_eq!(cargos.len(), 11);
    assert_eq!(cargos[0], CargoType::Passengers);
    assert_eq!(cargos[10], CargoType::Valuables);
    assert!(!cargos.contains(&CargoType::Gold));
    assert!(!cargos.contains(&CargoType::CottonCandy));
}

#[test]
fn arctic_wood_paper_goods_chain_golden() {
    let cargos = CargoType::for_climate(Climate::SubArctic);
    assert!(cargos.contains(&CargoType::Paper));
    assert!(cargos.contains(&CargoType::Gold));
    assert!(cargos.contains(&CargoType::Food));
    assert!(!cargos.contains(&CargoType::IronOre));

    let (wood, paper, pay) = chain_produce_process_pay(
        IndustrySpec::Forest,
        IndustrySpec::PaperMill,
        &[(CargoType::Wood, 16)],
        CargoType::Paper,
        8,
        40,
        5,
    );
    assert_eq!(wood, 13, "forest production_rate=13");
    assert_eq!(paper, 8, "paper mill wood→paper ×1");
    assert_eq!(pay, 212);
    assert_ne!(CargoType::Paper, CargoType::Steel);
}

#[test]
fn tropic_copper_factory_chain_golden() {
    let cargos = CargoType::for_climate(Climate::SubTropical);
    assert!(cargos.contains(&CargoType::Rubber));
    assert!(cargos.contains(&CargoType::CopperOre));
    assert!(cargos.contains(&CargoType::Diamonds));
    assert!(!cargos.contains(&CargoType::Coal));

    let mine = Industry::with_tiles_spec(
        TileCoord::new(0, 0),
        IndustryKind::CoalMine,
        IndustrySpec::CopperOreMine,
        vec![TileCoord::new(0, 0)],
        0,
    );
    assert_eq!(mine.output_cargo(), CargoType::CopperOre);
    assert_eq!(mine.produce_amount(), 10);

    let (rubber_out, goods, pay) = chain_produce_process_pay(
        IndustrySpec::RubberPlantation,
        IndustrySpec::FactoryTropic,
        &[
            (CargoType::Rubber, 16),
            (CargoType::CopperOre, 16),
            (CargoType::Wood, 16),
        ],
        CargoType::Goods,
        8,
        30,
        4,
    );
    assert_eq!(rubber_out, 10);
    assert_eq!(goods, 24, "three inputs ×8");
    assert_eq!(pay, 179);
}

#[test]
fn toyland_candy_chain_golden() {
    let cargos = CargoType::for_climate(Climate::Toyland);
    assert_eq!(cargos.len(), 12);
    assert_eq!(cargos[1], CargoType::Sugar);
    assert_eq!(cargos[8], CargoType::CottonCandy);
    assert!(!cargos.contains(&CargoType::Coal));
    assert!(!cargos.contains(&CargoType::Wood));

    let sugar = Industry::with_tiles_spec(
        TileCoord::new(0, 0),
        IndustryKind::CoalMine,
        IndustrySpec::SugarMine,
        vec![TileCoord::new(0, 0)],
        0,
    );
    assert_eq!(sugar.output_cargo(), CargoType::Sugar);

    let (_cc, candy, pay) = chain_produce_process_pay(
        IndustrySpec::CottonCandy,
        IndustrySpec::CandyFactory,
        &[
            (CargoType::Sugar, 16),
            (CargoType::Toffee, 16),
            (CargoType::CottonCandy, 16),
        ],
        CargoType::Candy,
        8,
        25,
        6,
    );
    assert_eq!(candy, 24);
    assert_eq!(pay, 148);
    let mut stock = crate::CargoStock::default();
    stock.add(CargoType::CottonCandy, 5);
    stock.add(CargoType::Batteries, 3);
    assert_eq!(stock.wood, 0);
    assert_eq!(stock.coal, 0);
    assert_eq!(stock.cotton_candy, 5);
    assert_eq!(stock.batteries, 3);
}

#[test]
fn arctic_gold_delivery_payment_golden() {
    let mut mine = Industry::with_tiles_spec(
        TileCoord::new(1, 1),
        IndustryKind::CoalMine,
        IndustrySpec::GoldMine,
        vec![TileCoord::new(1, 1)],
        0,
    );
    mine.produce(256);
    assert_eq!(mine.output_cargo(), CargoType::Gold);
    assert_eq!(mine.stock, 7);
    let pay = transported_goods_income(7, 50, 8, CargoType::Gold, 1 << 16);
    assert_eq!(pay, 246);
}

#[test]
fn save_roundtrip_preserves_climate_cargo_labels() {
    let mut stock = crate::CargoStock::default();
    stock.add(CargoType::Gold, 11);
    stock.add(CargoType::CottonCandy, 4);
    stock.add(CargoType::Rubber, 9);
    let json = serde_json::to_string(&stock).unwrap();
    let loaded: crate::CargoStock = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.gold, 11);
    assert_eq!(loaded.cotton_candy, 4);
    assert_eq!(loaded.rubber, 9);
    assert_eq!(loaded.wood, 0);
    assert_eq!(loaded.coal, 0);
}

#[test]
fn sav_slot_resolves_by_landscape() {
    assert_eq!(
        CargoType::from_climate_slot(Climate::Toyland, 1),
        Some(CargoType::Sugar)
    );
    assert_eq!(
        CargoType::from_climate_slot(Climate::Temperate, 1),
        Some(CargoType::Coal)
    );
    assert_eq!(
        CargoType::from_climate_slot(Climate::SubTropical, 8),
        Some(CargoType::CopperOre)
    );
    assert_eq!(
        CargoType::from_climate_slot(Climate::SubArctic, 10),
        Some(CargoType::Gold)
    );
}

#[test]
fn tropic_oil_wood_payment_differs_from_temperate() {
    let oil_temp = transported_goods_income(8, 30, 4, CargoType::Oil, 1 << 16);
    let oil_trop = transported_goods_income_for_climate(
        8,
        30,
        4,
        CargoType::Oil,
        Climate::SubTropical,
        1 << 16,
    );
    let wood_temp = transported_goods_income(8, 30, 4, CargoType::Wood, 1 << 16);
    let wood_trop = transported_goods_income_for_climate(
        8,
        30,
        4,
        CargoType::Wood,
        Climate::SubTropical,
        1 << 16,
    );
    assert_eq!(oil_temp, 129);
    assert_eq!(oil_trop, 142);
    assert_eq!(wood_temp, 146);
    assert_eq!(wood_trop, 232);
    assert!(oil_trop > oil_temp);
    assert!(wood_trop > wood_temp);
    assert_eq!(
        CargoType::Oil.payment_spec_for_climate(Climate::SubTropical).base_rate,
        4892
    );
    assert_eq!(
        CargoType::Wood.payment_spec_for_climate(Climate::SubTropical).base_rate,
        7964
    );
    assert_eq!(
        CargoType::Oil.payment_spec_for_climate(Climate::Temperate).base_rate,
        CargoType::Oil.payment_spec().base_rate
    );
}

#[test]
fn farm_dual_output_produces_and_stores_both() {
    let pos = TileCoord::new(2, 2);
    let mut farm = Industry::with_tiles_spec(
        pos,
        IndustryKind::Forest,
        IndustrySpec::Farm,
        vec![pos],
        0,
    );
    assert_eq!(
        farm.produced_cargos(),
        &[CargoType::Grain, CargoType::Livestock]
    );
    assert_eq!(farm.secondary_output_cargo(), Some(CargoType::Livestock));
    farm.produce(256);
    assert_eq!(farm.stock, 10, "grain production_rate=10");
    assert_eq!(farm.secondary_stock, 10, "livestock production_rate=10");
    assert_eq!(farm.output_cargo(), CargoType::Grain);
}
