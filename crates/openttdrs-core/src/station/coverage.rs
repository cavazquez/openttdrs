use crate::Climate;
use crate::airport_class::airport_spec_def;
use crate::cargo::{ALL_CARGO_TYPES, CargoStock, CargoType};
use crate::cargo_spec::CargoSpecDef;
use crate::house_spec::{STATION_ACCEPTANCE_THRESHOLD, add_accepted_cargo_of_house};
use crate::industry::{Industry, IndustryKind};
use crate::industry_spec::IndustrySpecDef;
use crate::industry_tile::{IndustryTileSpecDef, industry_tile_spec_def};
use crate::map::{Map, TileCoord, TileKind};
use crate::newgrf_callback::{
    IndustryTileCargoAcceptance,
    resolve_industry_tile_cargo_acceptance_callback_with_world_and_cargo_catalog,
};
use crate::town::Town;

use super::model::{Station, StopKind};

pub const STATION_COVERAGE_RADIUS: i32 = 4;

/// Radio de cobertura efectivo (`OpenTTD` catchment por `AirportSpec`, resto = 4).
#[must_use]
pub fn station_catchment_radius(station: &Station) -> i32 {
    if station.stop_kind == StopKind::Airport
        && let Some(def) = airport_spec_def(station.airport_spec)
    {
        return def.catchment.max(0);
    }
    STATION_COVERAGE_RADIUS
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StationCoverage {
    /// Teselas `House` dentro del radio (origen de pasajeros/correo).
    pub house_tiles: u32,
    /// Aceptación de correo en octavos (`AddAcceptedCargo_Town`).
    pub accepts_mail: u32,
    /// Aceptación de mercancías/comida en octavos.
    pub accepts_goods: u32,
    /// Aceptación de agua (proxy Oil) en octavos.
    pub accepts_water: u32,
    /// Aceptación de pasajeros en octavos.
    pub accepts_passengers: u32,
    pub supplies_coal: u32,
    pub supplies_wood: u32,
    pub supplies_oil: u32,
    pub supplied_stock: u32,
    /// Aceptación por cargo (en octavos), incluyendo teselas `IndustryTile`
    /// `NewGRF`. Se mantiene aparte de los cuatro contadores históricos para
    /// no perder cargos que no son `Goods`/`Water`.
    pub accepted_cargo: CargoStock,
    /// Cuando una tesela de industria `NewGRF` participa en la cobertura, la
    /// tabla por cargo es exacta: un cero de callback no debe caer al proxy
    /// genérico de mercancías.
    pub exact_cargo_acceptance: bool,
}

impl StationCoverage {
    #[must_use]
    pub fn accepts_anything(self) -> bool {
        self.accepts_mail >= STATION_ACCEPTANCE_THRESHOLD
            || self.accepts_goods >= STATION_ACCEPTANCE_THRESHOLD
            || self.accepts_water >= STATION_ACCEPTANCE_THRESHOLD
            || self.exact_cargo_acceptance
                && ALL_CARGO_TYPES
                    .iter()
                    .copied()
                    .any(|cargo| self.accepted_cargo.get(cargo) >= STATION_ACCEPTANCE_THRESHOLD)
            || self.exact_cargo_acceptance
                && self
                    .accepted_cargo
                    .custom_entries()
                    .any(|(_, amount)| amount >= STATION_ACCEPTANCE_THRESHOLD)
    }

    /// ¿La estación acepta mercancías urbanas? (`amt >= 8`).
    #[must_use]
    pub const fn accepts_town_goods(self) -> bool {
        self.accepts_goods >= STATION_ACCEPTANCE_THRESHOLD
    }

    #[must_use]
    pub const fn supplies_anything(self) -> bool {
        self.supplies_coal > 0 || self.supplies_wood > 0 || self.supplies_oil > 0
    }
}

/// Desajustes entre teselas `MP_STATION` y entradas en [`crate::game_state::GameState::stations`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StationMapCoherenceReport {
    pub tiles_without_station: Vec<TileCoord>,
    pub stations_without_tile: Vec<TileCoord>,
}

/// Comprueba que cada tesela `Station` tenga entrada en `state.stations` y viceversa.
#[must_use]
pub fn station_map_coherence(state: &crate::GameState) -> StationMapCoherenceReport {
    use std::collections::HashSet;

    let mut report = StationMapCoherenceReport::default();
    let state_positions: HashSet<(i32, i32)> =
        state.stations.iter().map(|s| (s.pos.x, s.pos.y)).collect();

    let (mw, mh) = state.map.dimensions();
    let mut tile_positions = HashSet::new();
    for y in 0..mh {
        for x in 0..mw {
            let c = TileCoord::new(x.cast_signed(), y.cast_signed());
            if state.map.get_kind(c) == Some(TileKind::Station) {
                tile_positions.insert((c.x, c.y));
                if !state_positions.contains(&(c.x, c.y)) {
                    report.tiles_without_station.push(c);
                }
            }
        }
    }

    for station in &state.stations {
        let key = (station.pos.x, station.pos.y);
        if !tile_positions.contains(&key) {
            report.stations_without_tile.push(station.pos);
        }
    }

    report
}

#[must_use]
pub const fn station_covers_tile(station_pos: TileCoord, tile: TileCoord, radius: i32) -> bool {
    (tile.x - station_pos.x).abs() <= radius && (tile.y - station_pos.y).abs() <= radius
}

#[must_use]
pub fn industry_in_station_coverage(
    industry: &Industry,
    station_pos: TileCoord,
    radius: i32,
) -> bool {
    industry
        .tiles
        .iter()
        .copied()
        .chain(std::iter::once(industry.pos))
        .any(|tile| station_covers_tile(station_pos, tile, radius))
}

#[must_use]
pub fn industry_in_station_coverage_by_pos(
    industry_pos: TileCoord,
    station_or_source: TileCoord,
    radius: i32,
) -> bool {
    station_covers_tile(station_or_source, industry_pos, radius)
}

/// Cobertura de una estación usando su catchment (aeropuerto por spec).
#[must_use]
pub fn station_coverage_for(
    map: &Map,
    industries: &[Industry],
    station: &Station,
) -> StationCoverage {
    station_coverage_at(
        map,
        industries,
        station.pos,
        station_catchment_radius(station),
    )
}

#[must_use]
pub fn station_coverage_at(
    map: &Map,
    industries: &[Industry],
    pos: TileCoord,
    radius: i32,
) -> StationCoverage {
    let mut coverage = StationCoverage::default();
    for y in pos.y - radius..=pos.y + radius {
        for x in pos.x - radius..=pos.x + radius {
            let c = TileCoord::new(x, y);
            let Some(tile) = map.get(c) else {
                continue;
            };
            match tile.kind {
                TileKind::House => {
                    coverage.house_tiles += 1;
                    let house_id = tile.m8 & 0x0FFF;
                    let mut amounts = [0u32; 5];
                    add_accepted_cargo_of_house(house_id, &mut amounts);
                    coverage.accepts_passengers =
                        coverage.accepts_passengers.saturating_add(amounts[0]);
                    coverage.accepts_mail = coverage.accepts_mail.saturating_add(amounts[1]);
                    coverage.accepts_goods = coverage.accepts_goods.saturating_add(amounts[2]);
                    coverage.accepts_water = coverage.accepts_water.saturating_add(amounts[3]);
                    coverage
                        .accepted_cargo
                        .add(CargoType::Passengers, amounts[0]);
                    coverage.accepted_cargo.add(CargoType::Mail, amounts[1]);
                    coverage.accepted_cargo.add(CargoType::Goods, amounts[2]);
                    // The reduced house table uses the historical Oil slot as
                    // the proxy for tropical Water acceptance.
                    coverage.accepted_cargo.add(CargoType::Oil, amounts[3]);
                }
                TileKind::Industry => {
                    coverage.accepts_goods = coverage
                        .accepts_goods
                        .saturating_add(STATION_ACCEPTANCE_THRESHOLD);
                    coverage
                        .accepted_cargo
                        .add(CargoType::Goods, STATION_ACCEPTANCE_THRESHOLD);
                }
                _ => {}
            }
        }
    }

    for industry in industries {
        if !industry_in_station_coverage(industry, pos, radius) {
            continue;
        }
        coverage.supplied_stock = coverage.supplied_stock.saturating_add(industry.stock);
        match industry.kind {
            IndustryKind::CoalMine => coverage.supplies_coal += 1,
            IndustryKind::Forest => coverage.supplies_wood += 1,
            IndustryKind::OilWell => coverage.supplies_oil += 1,
            IndustryKind::Factory => {
                coverage.accepts_goods = coverage
                    .accepts_goods
                    .saturating_add(STATION_ACCEPTANCE_THRESHOLD);
                coverage
                    .accepted_cargo
                    .add(CargoType::Goods, STATION_ACCEPTANCE_THRESHOLD);
            }
        }
    }

    coverage
}

fn add_industry_tile_acceptance(
    coverage: &mut StationCoverage,
    acceptance: IndustryTileCargoAcceptance,
) {
    for (cargo, amount) in acceptance.cargos.into_iter().zip(acceptance.amounts) {
        let Some(cargo) = cargo else {
            continue;
        };
        let Ok(amount) = u32::try_from(amount) else {
            continue;
        };
        coverage.accepted_cargo.add(cargo, amount);
        match cargo {
            CargoType::Passengers => {
                coverage.accepts_passengers = coverage.accepts_passengers.saturating_add(amount);
            }
            CargoType::Mail => {
                coverage.accepts_mail = coverage.accepts_mail.saturating_add(amount);
            }
            CargoType::Goods => {
                coverage.accepts_goods = coverage.accepts_goods.saturating_add(amount);
            }
            CargoType::Oil | CargoType::Water => {
                coverage.accepts_water = coverage.accepts_water.saturating_add(amount);
            }
            _ => {}
        }
    }
}

fn industry_for_tile(industries: &[Industry], map: &Map, coord: TileCoord) -> Option<usize> {
    let tile = map.get(coord)?;
    if tile.kind != TileKind::Industry {
        return None;
    }
    let instance_id = crate::map::industry_instance_id(&tile);
    industries.iter().position(|industry| {
        industry.contains_tile(coord) && (instance_id == 0 || industry.instance_id == instance_id)
    })
}

/// Cobertura de una estación con la regla nativa de aceptación por tesela de
/// industria (`CBID_INDTILE_ACCEPT_CARGO`/`CBID_INDTILE_CARGO_ACCEPTANCE`).
///
/// Las APIs históricas (`station_coverage_at`/`station_coverage_for`) siguen
/// siendo inmutables y conservan sus proxies vanilla. Esta variante se usa en
/// los puntos de simulación que tienen el catálogo y el pool vivo: el callback
/// puede escribir PSA en la industria parent y el resultado por cargo evita
/// aceptar `Goods` sólo por el proxy genérico cuando el GRF devolvió cero.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn station_coverage_at_with_newgrf(
    map: &Map,
    industries: &mut [Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    pos: TileCoord,
    radius: i32,
) -> StationCoverage {
    station_coverage_at_with_newgrf_and_cargo_catalog(
        map,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
        pos,
        radius,
        &[],
    )
}

/// Variante catálogo-aware de [`station_coverage_at_with_newgrf`].
///
/// El catálogo se entrega al callback de aceptación de cada `IndustryTile`
/// para que labels custom se puedan resolver aunque el SAV no haya hidratado
/// los slots de cargos de la industria parent.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn station_coverage_at_with_newgrf_and_cargo_catalog(
    map: &Map,
    industries: &mut [Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    pos: TileCoord,
    radius: i32,
    cargo_catalog: &[CargoSpecDef],
) -> StationCoverage {
    let mut coverage = StationCoverage::default();
    let mut snapshot = industries.to_vec();
    let mut exact_industries = vec![false; industries.len()];

    for y in pos.y - radius..=pos.y + radius {
        for x in pos.x - radius..=pos.x + radius {
            let c = TileCoord::new(x, y);
            let Some(tile) = map.get(c) else {
                continue;
            };
            match tile.kind {
                TileKind::House => {
                    coverage.house_tiles += 1;
                    let house_id = tile.m8 & 0x0FFF;
                    let mut amounts = [0u32; 5];
                    add_accepted_cargo_of_house(house_id, &mut amounts);
                    coverage.accepts_passengers =
                        coverage.accepts_passengers.saturating_add(amounts[0]);
                    coverage.accepts_mail = coverage.accepts_mail.saturating_add(amounts[1]);
                    coverage.accepts_goods = coverage.accepts_goods.saturating_add(amounts[2]);
                    coverage.accepts_water = coverage.accepts_water.saturating_add(amounts[3]);
                    coverage
                        .accepted_cargo
                        .add(CargoType::Passengers, amounts[0]);
                    coverage.accepted_cargo.add(CargoType::Mail, amounts[1]);
                    coverage.accepted_cargo.add(CargoType::Goods, amounts[2]);
                    coverage.accepted_cargo.add(CargoType::Oil, amounts[3]);
                }
                TileKind::Industry => {
                    let gfx = crate::map::industry_gfx(&tile);
                    let Some(def) = industry_tile_spec_def(tile_spec_catalog, gfx) else {
                        coverage.accepts_goods = coverage
                            .accepts_goods
                            .saturating_add(STATION_ACCEPTANCE_THRESHOLD);
                        coverage
                            .accepted_cargo
                            .add(CargoType::Goods, STATION_ACCEPTANCE_THRESHOLD);
                        continue;
                    };
                    coverage.exact_cargo_acceptance |= def.from_newgrf;
                    let acceptance = if let Some(index) = industry_for_tile(&snapshot, map, c) {
                        if def.from_newgrf {
                            exact_industries[index] = true;
                        }
                        let value = resolve_industry_tile_cargo_acceptance_callback_with_world_and_cargo_catalog(
                            def,
                            &mut industries[index],
                            map,
                            c,
                            &snapshot,
                            towns,
                            tile_spec_catalog,
                            industry_catalog,
                            climate,
                            cargo_catalog,
                        );
                        snapshot[index] = industries[index].clone();
                        value
                    } else {
                        IndustryTileCargoAcceptance::default()
                    };
                    add_industry_tile_acceptance(&mut coverage, acceptance);
                }
                _ => {}
            }
        }
    }

    for (index, industry) in industries.iter().enumerate() {
        if !industry_in_station_coverage(industry, pos, radius) {
            continue;
        }
        coverage.supplied_stock = coverage.supplied_stock.saturating_add(industry.stock);
        match industry.kind {
            IndustryKind::CoalMine => coverage.supplies_coal += 1,
            IndustryKind::Forest => coverage.supplies_wood += 1,
            IndustryKind::OilWell => coverage.supplies_oil += 1,
            IndustryKind::Factory if !exact_industries[index] => {
                coverage.accepts_goods = coverage
                    .accepts_goods
                    .saturating_add(STATION_ACCEPTANCE_THRESHOLD);
                coverage
                    .accepted_cargo
                    .add(CargoType::Goods, STATION_ACCEPTANCE_THRESHOLD);
            }
            IndustryKind::Factory => {}
        }
    }
    coverage
}

/// Decide si una parada admite un cargo usando la tabla de aceptación por
/// tesela. El predicado de tipo de parada se conserva como primera barrera;
/// sólo la cantidad de catchment cambia con `NewGRF`.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn station_accepts_cargo_with_newgrf(
    map: &Map,
    industries: &mut [Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    station: &Station,
    cargo: CargoType,
) -> bool {
    station_accepts_cargo_with_newgrf_and_cargo_catalog(
        map,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
        station,
        cargo,
        &[],
    )
}

/// Variante catálogo-aware de [`station_accepts_cargo_with_newgrf`].
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn station_accepts_cargo_with_newgrf_and_cargo_catalog(
    map: &Map,
    industries: &mut [Industry],
    towns: &[Town],
    tile_spec_catalog: &[IndustryTileSpecDef],
    industry_catalog: &[IndustrySpecDef],
    climate: Climate,
    station: &Station,
    cargo: CargoType,
    cargo_catalog: &[CargoSpecDef],
) -> bool {
    if !station.accepts_cargo(cargo) {
        return false;
    }
    let coverage = station_coverage_at_with_newgrf_and_cargo_catalog(
        map,
        industries,
        towns,
        tile_spec_catalog,
        industry_catalog,
        climate,
        station.pos,
        station_catchment_radius(station),
        cargo_catalog,
    );
    if coverage.exact_cargo_acceptance {
        coverage.accepted_cargo.get(cargo) >= STATION_ACCEPTANCE_THRESHOLD
    } else {
        // The legacy unload path only had the station-type predicate. Keep
        // that behavior when no NewGRF industry tile supplied an exact table;
        // the catchment amounts remain available to Action2/UI callers.
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::airport_class::AirportSpecId;
    use crate::map::Map;
    use crate::newgrf_sprites::{
        Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::station::Station;

    fn industry_tile_acceptance_runtime(
        cargo_result: u16,
        amount_result: u16,
    ) -> TrainSpriteGraphics {
        let literal = |value: u16| Action2VarTerm {
            variable: 0x1A,
            param: None,
            adjust: Action2VarAdjust {
                and_mask: u32::from(value),
                ..Action2VarAdjust::default()
            },
        };
        let mut runtime = TrainSpriteGraphics::default();
        runtime.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 2,
        });
        runtime.action2_var.insert(
            2,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x0C,
                    param: None,
                    adjust: Action2VarAdjust {
                        and_mask: u32::from(u16::MAX),
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: vec![
                    (
                        3,
                        u32::from(crate::newgrf_sprites::CBID_INDTILE_ACCEPT_CARGO),
                        u32::from(crate::newgrf_sprites::CBID_INDTILE_ACCEPT_CARGO),
                    ),
                    (
                        4,
                        u32::from(crate::newgrf_sprites::CBID_INDTILE_CARGO_ACCEPTANCE),
                        u32::from(crate::newgrf_sprites::CBID_INDTILE_CARGO_ACCEPTANCE),
                    ),
                ],
                default: 0,
            },
        );
        runtime.action2_var.insert(
            3,
            Action2VarEntry {
                first: literal(cargo_result),
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        runtime.action2_var.insert(
            4,
            Action2VarEntry {
                first: literal(amount_result),
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        runtime
    }

    #[test]
    fn heliport_and_small_catchment_are_four() {
        let mut heli = Station::new_with_kind(TileCoord::new(10, 10), StopKind::Airport);
        heli.airport_spec = AirportSpecId::Heliport;
        assert_eq!(station_catchment_radius(&heli), 4);

        let mut small = Station::new_with_kind(TileCoord::new(10, 10), StopKind::Airport);
        small.airport_spec = AirportSpecId::Small;
        assert_eq!(station_catchment_radius(&small), 4);
    }

    #[test]
    fn intercontinental_catchment_is_ten() {
        let mut st = Station::new_with_kind(TileCoord::new(20, 20), StopKind::Airport);
        st.airport_spec = AirportSpecId::Intercontinental;
        assert_eq!(station_catchment_radius(&st), 10);
    }

    #[test]
    fn intercontinental_covers_houses_beyond_default_radius() {
        let mut map = Map::new_flat(64, 64, 1);
        let airport_pos = TileCoord::new(20, 20);
        let far_house = TileCoord::new(20, 27); // chebyshev dist 7 (> 4, < 10)
        let mut house = map.get(far_house).expect("tile");
        house.kind = TileKind::House;
        map.set_tile(far_house, house).unwrap();

        let mut heli = Station::new_with_kind(airport_pos, StopKind::Airport);
        heli.airport_spec = AirportSpecId::Heliport;
        let heli_cov = station_coverage_for(&map, &[], &heli);
        assert_eq!(heli_cov.house_tiles, 0, "Heliport r=4 no alcanza dist 7");

        let mut inter = Station::new_with_kind(airport_pos, StopKind::Airport);
        inter.airport_spec = AirportSpecId::Intercontinental;
        let inter_cov = station_coverage_for(&map, &[], &inter);
        assert_eq!(
            inter_cov.house_tiles, 1,
            "Intercontinental r=10 debe cubrir casa a dist 7"
        );
    }

    #[test]
    fn rail_station_keeps_default_catchment() {
        let st = Station::new_with_kind(TileCoord::new(5, 5), StopKind::RailStation);
        assert_eq!(station_catchment_radius(&st), STATION_COVERAGE_RADIUS);
    }

    #[test]
    fn house_spec_acceptance_feeds_catchment_goods() {
        let mut map = Map::new_flat(16, 16, 0);
        let house = TileCoord::new(8, 8);
        map.set_completed_house(house, 0, 0).unwrap(); // office: goods 4/8
        let cov = station_coverage_at(&map, &[], TileCoord::new(8, 8), 1);
        assert_eq!(cov.house_tiles, 1);
        assert_eq!(cov.accepts_passengers, 8);
        assert_eq!(cov.accepts_mail, 3);
        assert_eq!(cov.accepts_goods, 4);
        assert!(!cov.accepts_town_goods(), "hace falta ≥8/8");

        map.set_completed_house(TileCoord::new(8, 9), 0, 0).unwrap();
        let cov2 = station_coverage_at(&map, &[], TileCoord::new(8, 8), 1);
        assert_eq!(cov2.accepts_goods, 8);
        assert!(cov2.accepts_town_goods());
    }

    #[test]
    fn newgrf_industry_tile_acceptance_controls_unload_cargo() {
        let coord = TileCoord::new(8, 8);
        let mut map = Map::new_flat(16, 16, 0);
        let mut tile = map.get(coord).expect("tile");
        tile.kind = TileKind::Industry;
        tile.m1 = 0x80;
        tile.m2 = 7;
        crate::map::set_industry_gfx(&mut tile, 175);
        map.set_tile(coord, tile).unwrap();

        let def = IndustryTileSpecDef {
            gfx: crate::industry_tile::IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: vec![5],
            accepts_cargo_labels: vec!["GOOD".into()],
            acceptance: vec![8],
            callback_mask: crate::industry_tile::INDUSTRY_TILE_CALLBACK_ACCEPT_CARGO_MASK
                | crate::industry_tile::INDUSTRY_TILE_CALLBACK_CARGO_ACCEPTANCE_MASK,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 0,
            newgrf_grfid: 1,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(industry_tile_acceptance_runtime(1, 8))),
        };
        let mut industries = vec![Industry::new(coord, IndustryKind::Factory).with_instance_id(7)];
        let station = Station::new_with_kind(TileCoord::new(9, 8), StopKind::TruckStop);
        let catalog = vec![def];

        assert!(station_accepts_cargo_with_newgrf(
            &map,
            &mut industries,
            &[],
            &catalog,
            &[],
            Climate::Temperate,
            &station,
            CargoType::Coal,
        ));
        assert!(
            !station_accepts_cargo_with_newgrf(
                &map,
                &mut industries,
                &[],
                &catalog,
                &[],
                Climate::Temperate,
                &station,
                CargoType::Goods,
            ),
            "CB2C reemplaza Goods por Coal; no debe quedar el proxy genérico"
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn newgrf_industry_tile_acceptance_can_return_custom_cargo() {
        let coord = TileCoord::new(8, 8);
        let mut map = Map::new_flat(16, 16, 0);
        let mut tile = map.get(coord).expect("tile");
        tile.kind = TileKind::Industry;
        tile.m1 = 0x80;
        tile.m2 = 7;
        crate::map::set_industry_gfx(&mut tile, 175);
        map.set_tile(coord, tile).unwrap();

        let tile_def = IndustryTileSpecDef {
            gfx: crate::industry_tile::IndustryTileGfxId(175),
            subst_id: 0,
            from_newgrf: true,
            slopes_refused: 0,
            accepts_cargo_indices: vec![5],
            accepts_cargo_labels: vec!["GOOD".into()],
            acceptance: vec![8],
            callback_mask: crate::industry_tile::INDUSTRY_TILE_CALLBACK_ACCEPT_CARGO_MASK
                | crate::industry_tile::INDUSTRY_TILE_CALLBACK_CARGO_ACCEPTANCE_MASK,
            animation_frames: 0,
            animation_status: 0,
            animation_speed: 0,
            animation_triggers: 0,
            animation_special_flags: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            newgrf_local_id: 0,
            newgrf_grfid: 1,
            newgrf_preview: None,
            newgrf_views: Vec::new(),
            newgrf_runtime: Some(Box::new(industry_tile_acceptance_runtime(3, 8))),
        };
        let cargo_catalog = vec![crate::CargoSpecDef {
            id: crate::cargo::CUSTOM_CARGO_OFFSET,
            local_id: 3,
            label: "TOFU".into(),
            from_newgrf: true,
            grfid: 1,
            is_freight: true,
            ..crate::CargoSpecDef::default()
        }];
        let industry_def = crate::IndustrySpecDef {
            id: 7,
            local_id: 0,
            subst_id: 0,
            override_id: None,
            layouts: Vec::new(),
            produced_cargo_indices: Vec::new(),
            produced_cargo_labels: Vec::new(),
            accepted_cargo_indices: vec![3],
            accepted_cargo_labels: vec!["TOFU".into()],
            production_rates: Vec::new(),
            input_multipliers: Vec::new(),
            callback_mask: 0,
            behaviour: 0,
            cost_multiplier: 0,
            associated_badges: Vec::new(),
            newgrf_badge_translation: Vec::new(),
            name: "Tofu plant".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_runtime: None,
        };
        let custom = CargoType::Custom(0);
        let station = Station::new_with_kind(TileCoord::new(9, 8), StopKind::TruckStop);
        let mut legacy_industries = vec![
            Industry::new(coord, IndustryKind::Factory)
                .with_instance_id(7)
                .with_newgrf_spec(7, &industry_def),
        ];
        assert!(
            !station_accepts_cargo_with_newgrf(
                &map,
                &mut legacy_industries,
                &[],
                std::slice::from_ref(&tile_def),
                std::slice::from_ref(&industry_def),
                Climate::Temperate,
                &station,
                custom,
            ),
            "sin catálogo, el label custom del parent no puede resolverse"
        );

        let mut industries = vec![
            Industry::new(coord, IndustryKind::Factory)
                .with_instance_id(7)
                .with_newgrf_spec(7, &industry_def),
        ];
        assert!(station_accepts_cargo_with_newgrf_and_cargo_catalog(
            &map,
            &mut industries,
            &[],
            std::slice::from_ref(&tile_def),
            std::slice::from_ref(&industry_def),
            Climate::Temperate,
            &station,
            custom,
            &cargo_catalog,
        ));
        assert!(
            !station_accepts_cargo_with_newgrf_and_cargo_catalog(
                &map,
                &mut industries,
                &[],
                &[IndustryTileSpecDef {
                    accepts_cargo_indices: vec![5],
                    accepts_cargo_labels: vec!["GOOD".into()],
                    newgrf_runtime: Some(Box::new(industry_tile_acceptance_runtime(3, 0))),
                    ..tile_def.clone()
                }],
                std::slice::from_ref(&industry_def),
                Climate::Temperate,
                &station,
                CargoType::Goods,
                &cargo_catalog,
            ),
            "el callback custom no debe volver a aceptar Goods por proxy"
        );
    }
}
