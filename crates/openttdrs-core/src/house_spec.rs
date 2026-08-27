//! Specs de casas vanilla (`HouseSpec` / `_original_house_specs`) y `NewGRF` (`HouseSpecDef`).
//!
//! Datos generados en el módulo privado `crate::sav::house_population_generated`; aquí viven
//! las consultas runtime para zonas, años, pesos y aceptación (P3.5–P3.7). Feature Action0 `0x07`.

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::map::{Tile, TileCoord};
use crate::sav::house_population_generated::{
    HOUSE_ACCEPTS_CARGO, HOUSE_AVAILABILITY, HOUSE_BUILDING_FLAGS, HOUSE_CARGO_ACCEPTANCE,
    HOUSE_MAIL_GENERATION, HOUSE_MAX_YEAR, HOUSE_MAX_YEAR_OF, HOUSE_MIN_YEAR, HOUSE_MINIMUM_LIFE,
    HOUSE_POPULATION, HOUSE_PROBABILITY, HOUSE_SIZE_1X1, HOUSE_SPEC_COUNT,
};
use crate::town::{HouseZone, NUM_HOUSE_ZONES, Town, TownLayout};
use crate::world_gen::{Climate, DEF_SNOW_LINE_HEIGHT};

/// Número de `HouseID` vanilla.
pub const NUM_HOUSES_VANILLA: usize = HOUSE_SPEC_COUNT;
pub const HOUSE_YEAR_MAX: u32 = HOUSE_MAX_YEAR;

/// Primer `HouseID` definido por `NewGRF` (`OpenTTD` `NEW_HOUSE_OFFSET`).
pub const NEW_HOUSE_OFFSET: u16 = 110;
/// Total de slots de casa (`OpenTTD` `NUM_HOUSES` histórico).
pub const NUM_HOUSES: u16 = 512;
/// Id inválido / sin override.
pub const INVALID_HOUSE: u16 = NUM_HOUSES;
/// Probabilidad por defecto Action0 si no hay prop `0x18`.
pub const DEFAULT_HOUSE_PROBABILITY: u8 = 16;
/// Availability por defecto (todas las zonas + climas).
pub const DEFAULT_HOUSE_AVAILABILITY: u16 = 0xFFFF;

/// Flags de edificio (`BuildingFlag`).
pub const BUILDING_FLAG_SIZE_1X1: u8 = 1 << 0;
pub const BUILDING_FLAG_NOT_SLOPED: u8 = 1 << 1;
pub const BUILDING_FLAG_SIZE_2X1: u8 = 1 << 2;
pub const BUILDING_FLAG_SIZE_1X2: u8 = 1 << 3;
pub const BUILDING_FLAG_SIZE_2X2: u8 = 1 << 4;
pub const BUILDING_FLAG_IS_ANIMATED: u8 = 1 << 5;
pub const BUILDING_FLAG_IS_CHURCH: u8 = 1 << 6;
pub const BUILDING_FLAG_IS_STADIUM: u8 = 1 << 7;

/// Umbral de aceptación de estación en octavos (`amt >= 8`).
pub const STATION_ACCEPTANCE_THRESHOLD: u32 = 8;
/// Bit `HouseCallbackMask::AllowConstruction`: consulta CB `0x17` al crecer.
pub const HOUSE_CALLBACK_ALLOW_CONSTRUCTION_MASK: u16 = 1;

/// Vista de un `HouseSpec` vanilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HouseSpec {
    pub id: u16,
    pub min_year: u32,
    pub max_year: u32,
    pub population: u16,
    pub mail_generation: u16,
    pub probability: u8,
    pub minimum_life: u8,
    pub building_flags: u8,
    pub availability: u16,
    pub cargo_acceptance: [u8; 3],
    pub accepts_cargo: [u8; 3],
}

impl HouseSpec {
    #[must_use]
    pub fn get(house_id: u16) -> Option<Self> {
        let i = usize::from(house_id);
        if i >= HOUSE_SPEC_COUNT {
            return None;
        }
        Some(Self {
            id: house_id,
            min_year: HOUSE_MIN_YEAR[i],
            max_year: HOUSE_MAX_YEAR_OF[i],
            population: HOUSE_POPULATION[i],
            mail_generation: HOUSE_MAIL_GENERATION[i],
            probability: HOUSE_PROBABILITY[i],
            minimum_life: HOUSE_MINIMUM_LIFE[i],
            building_flags: HOUSE_BUILDING_FLAGS[i],
            availability: HOUSE_AVAILABILITY[i],
            cargo_acceptance: HOUSE_CARGO_ACCEPTANCE[i],
            accepts_cargo: HOUSE_ACCEPTS_CARGO[i],
        })
    }

    #[must_use]
    pub const fn is_size_1x1(self) -> bool {
        self.building_flags & BUILDING_FLAG_SIZE_1X1 != 0
            && self.building_flags
                & (BUILDING_FLAG_SIZE_2X1 | BUILDING_FLAG_SIZE_1X2 | BUILDING_FLAG_SIZE_2X2)
                == 0
    }

    #[must_use]
    pub const fn is_church(self) -> bool {
        self.building_flags & BUILDING_FLAG_IS_CHURCH != 0
    }

    #[must_use]
    pub const fn is_stadium(self) -> bool {
        self.building_flags & BUILDING_FLAG_IS_STADIUM != 0
    }

    #[must_use]
    pub const fn requires_flat(self) -> bool {
        self.building_flags & BUILDING_FLAG_NOT_SLOPED != 0
    }

    /// ¿El spec admite la zona/clima pedidos? (`building_availability.All(zones)`).
    #[must_use]
    pub const fn matches_zones(self, required: u16) -> bool {
        self.availability & required == required
    }
}

/// Spec `NewGRF` de casa (feature Action0 `0x07`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseSpecDef {
    /// Id global (`≥` [`NEW_HOUSE_OFFSET`] si `from_newgrf`).
    pub id: u16,
    pub local_id: u8,
    /// Substitute vanilla (`prop 0x08`) para dibujo / fallback.
    pub subst_id: u8,
    /// Building flags (`prop 0x09`).
    pub building_flags: u8,
    pub min_year: u32,
    pub max_year: u32,
    pub population: u16,
    pub mail_generation: u16,
    /// Zonas + climas (`prop 0x13`).
    pub availability: u16,
    pub probability: u8,
    /// Override de casa vanilla (`prop 0x15`).
    pub override_id: Option<u8>,
    /// Callback mask (`0x14` lo + `0x1D` hi); CB17 se ejecuta al construir.
    pub callback_mask: u16,
    pub name: String,
    pub from_newgrf: bool,
    pub grfid: u32,
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    #[serde(default, skip)]
    pub newgrf_local_id: u8,
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
}

impl HouseSpecDef {
    /// ¿El GRF declaró CB `0x17` para autorizar la construcción de la casa?
    #[must_use]
    pub const fn has_construction_callback(&self) -> bool {
        self.callback_mask & HOUSE_CALLBACK_ALLOW_CONSTRUCTION_MASK != 0
    }

    #[must_use]
    pub const fn is_size_1x1(&self) -> bool {
        self.building_flags & BUILDING_FLAG_SIZE_1X1 != 0
            && self.building_flags
                & (BUILDING_FLAG_SIZE_2X1 | BUILDING_FLAG_SIZE_1X2 | BUILDING_FLAG_SIZE_2X2)
                == 0
    }

    #[must_use]
    pub const fn is_church(&self) -> bool {
        self.building_flags & BUILDING_FLAG_IS_CHURCH != 0
    }

    #[must_use]
    pub const fn is_stadium(&self) -> bool {
        self.building_flags & BUILDING_FLAG_IS_STADIUM != 0
    }

    #[must_use]
    pub const fn matches_zones(&self, required: u16) -> bool {
        self.availability & required == required
    }

    /// ¿Es la tesela norte de un multitile (flags de tamaño >1×1)?
    #[must_use]
    pub const fn is_multitile_north(&self) -> bool {
        self.building_flags
            & (BUILDING_FLAG_SIZE_2X1 | BUILDING_FLAG_SIZE_1X2 | BUILDING_FLAG_SIZE_2X2)
            != 0
    }

    #[must_use]
    pub fn has_newgrf_sprites(&self) -> bool {
        !self.newgrf_views.is_empty() || self.newgrf_runtime.is_some()
    }

    #[must_use]
    pub fn newgrf_view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return None;
        }
        self.newgrf_views.get(idx % self.newgrf_views.len())
    }

    /// Vista re-resolviendo Action2 con el contexto de la tesela.
    ///
    /// Las casas conservan sus bits aleatorios y de triggers en `m1`/`m3`;
    /// cuando el grupo usa una variational/random, el renderer debe volver a
    /// recorrer el grafo por cada contexto en vez de reutilizar el preview
    /// estático cargado al iniciar el GRF.
    pub fn newgrf_view_runtime(
        &self,
        idx: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::DecodedSprite> {
        let runtime = self.newgrf_runtime.as_ref()?;
        let views = runtime.views_for_local_id_ctx(self.newgrf_local_id, ctx)?;
        if views.is_empty() {
            return None;
        }
        Some(views[idx % views.len()].clone())
    }

    /// Layout `TileSeq` de Action2 para una tesela de casa.
    ///
    /// La etapa se utiliza para seleccionar la rama Action2; una referencia
    /// de layout ya apunta al primer sprite de su set Action1 y por eso no se
    /// vuelve a aplicar `idx` como desplazamiento de textura.
    pub fn newgrf_tile_layout_runtime(
        &self,
        idx: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::ResolvedTileLayout> {
        let _ = idx;
        self.newgrf_runtime.as_ref()?.tile_layout_for_local_id_ctx(
            u16::from(self.newgrf_local_id),
            0,
            ctx,
        )
    }
}

/// Construye el contexto Action2 disponible para una casa ya materializada.
///
/// Es la traducción de las variables de `HouseScopeResolver` que están
/// persistidas en `Tile`: etapa/hash (`0x40`), edad (`0x41`), terreno (`0x43`),
/// frame (`0x46`), posición (`0x47`) y random/triggers (`0x5F`). Las variables
/// que dependen del pueblo o de contadores globales (zona, número de casas,
/// vecinos y aceptación de estaciones) se dejan ausentes para que el
/// evaluador no invente valores.
#[must_use]
pub fn action2_eval_ctx_for_house_tile(
    tile: Tile,
    tx: i32,
    ty: i32,
    climate: Climate,
) -> crate::newgrf_sprites::Action2EvalCtx {
    let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
    let stage = if tile.m3 & 0x80 != 0 {
        3
    } else {
        u32::from((tile.m5 >> 3) & 0x03)
    };
    let tile_hash = (tx.cast_unsigned() ^ (tx.cast_unsigned() >> 2) ^ ty.cast_unsigned())
        .wrapping_sub(ty.cast_unsigned() >> 2)
        & 0x03;
    ctx.vars.insert(0x40, stage | tile_hash << 2);
    let age = if tile.m3 & 0x80 != 0 {
        u32::from(tile.m5)
    } else {
        0
    };
    ctx.vars.insert(0x41, age);
    // `GetTerrainType` returns a small climate-dependent enum. The map
    // representation has no separate terrain enum, but `m7` carries the
    // snow/desert marker used by imported maps; keep temperate grass as zero.
    let terrain = if climate.uses_desert_patches() && tile.m7 & 0x20 != 0 {
        1
    } else if climate.uses_snow_ground() || tile.m7 & 0x20 != 0 {
        4
    } else {
        0
    };
    ctx.vars.insert(0x43, terrain);
    ctx.vars.insert(0x46, u32::from(tile.m3hi));
    ctx.vars.insert(
        0x47,
        (ty.cast_unsigned() << 16) | (tx.cast_unsigned() & 0xFFFF),
    );
    ctx.random_bits = u32::from(tile.m1);
    // `var 5F` combines GetHouseRandomBits (high byte) and the pending
    // randomisation triggers (low five bits of m3).
    ctx.vars
        .insert(0x5F, (u32::from(tile.m1) << 8) | u32::from(tile.m3 & 0x1F));
    ctx
}

/// Catálogo vacío (solo `NewGRF`).
#[must_use]
pub fn empty_house_spec_catalog() -> Vec<HouseSpecDef> {
    Vec::new()
}

/// Tabla de overrides vanilla → id `NewGRF` (`prop 0x15`).
#[must_use]
pub fn empty_house_overrides() -> Vec<u16> {
    vec![INVALID_HOUSE; NEW_HOUSE_OFFSET as usize]
}

#[must_use]
pub fn house_spec_def(catalog: &[HouseSpecDef], id: u16) -> Option<&HouseSpecDef> {
    catalog.iter().find(|d| d.id == id)
}

/// Lookup vanilla o `NewGRF` por id global.
#[must_use]
pub fn vanilla_or_newgrf_house(catalog: &[HouseSpecDef], id: u16) -> Option<HouseLookup<'_>> {
    if let Some(def) = house_spec_def(catalog, id) {
        return Some(HouseLookup::NewGrf(def));
    }
    HouseSpec::get(id).map(HouseLookup::Vanilla)
}

/// Vista unificada vanilla / `NewGRF` para población y flags.
#[derive(Debug, Clone, Copy)]
pub enum HouseLookup<'a> {
    Vanilla(HouseSpec),
    NewGrf(&'a HouseSpecDef),
}

impl HouseLookup<'_> {
    #[must_use]
    pub const fn population(self) -> u16 {
        match self {
            Self::Vanilla(hs) => hs.population,
            Self::NewGrf(d) => d.population,
        }
    }

    #[must_use]
    pub const fn is_church(self) -> bool {
        match self {
            Self::Vanilla(hs) => hs.is_church(),
            Self::NewGrf(d) => d.is_church(),
        }
    }

    #[must_use]
    pub const fn is_stadium(self) -> bool {
        match self {
            Self::Vanilla(hs) => hs.is_stadium(),
            Self::NewGrf(d) => d.is_stadium(),
        }
    }

    #[must_use]
    pub const fn building_flags(self) -> u8 {
        match self {
            Self::Vanilla(hs) => hs.building_flags,
            Self::NewGrf(d) => d.building_flags,
        }
    }
}

/// Siguiente id libre en `[NEW_HOUSE_OFFSET, NUM_HOUSES)`.
#[must_use]
pub fn next_free_house_id(catalog: &[HouseSpecDef]) -> Option<u16> {
    (NEW_HOUSE_OFFSET..NUM_HOUSES).find(|&id| !catalog.iter().any(|d| d.id == id))
}

/// Offsets del footprint multitile (norte = `(0,0)`).
///
/// Orden OTTD: 2×2 → N, E `(1,0)`, W `(0,1)`, S `(1,1)`.
#[must_use]
pub fn house_footprint_offsets(building_flags: u8) -> Vec<(i32, i32)> {
    if building_flags & BUILDING_FLAG_SIZE_2X2 != 0 {
        return vec![(0, 0), (1, 0), (0, 1), (1, 1)];
    }
    if building_flags & BUILDING_FLAG_SIZE_2X1 != 0 {
        return vec![(0, 0), (1, 0)];
    }
    if building_flags & BUILDING_FLAG_SIZE_1X2 != 0 {
        return vec![(0, 0), (0, 1)];
    }
    vec![(0, 0)]
}

/// Id de dibujo vanilla: vistas `NewGRF` → el propio id; si no, `subst_id`; si no, `% 110`.
#[must_use]
pub fn resolve_house_draw_id(house_id: u16, catalog: &[HouseSpecDef]) -> u16 {
    let clean = house_id & 0xFFF;
    if let Some(def) = house_spec_def(catalog, clean) {
        if def.has_newgrf_sprites() {
            return clean;
        }
        return u16::from(def.subst_id);
    }
    if clean >= NEW_HOUSE_OFFSET {
        return clean % NEW_HOUSE_OFFSET;
    }
    clean
}

/// Traduce id vanilla aplicando override `NewGRF` (si hay).
#[must_use]
pub fn get_translated_house_id(clean: u16, overrides: &[u16]) -> u16 {
    if let Some(&ovr) = overrides.get(usize::from(clean))
        && ovr != INVALID_HOUSE
    {
        return ovr;
    }
    clean
}

/// Máscara de clima para el landscape actual (`GetClimateMaskForLandscape`).
#[must_use]
pub fn climate_zone_mask(climate: Climate, tile_height: u8) -> u16 {
    match climate {
        Climate::Temperate => 1 << (HouseZone::ClimateTemperate as u8),
        Climate::SubArctic => {
            if i32::from(tile_height) > i32::from(DEF_SNOW_LINE_HEIGHT) {
                1 << (HouseZone::ClimateSubarcticAboveSnow as u8)
            } else {
                1 << (HouseZone::ClimateSubarcticBelowSnow as u8)
            }
        }
        Climate::SubTropical => 1 << (HouseZone::ClimateSubtropic as u8),
        Climate::Toyland => 1 << (HouseZone::ClimateToyland as u8),
    }
}

/// Convierte el índice de aceptación generado a [`CargoType`] del port.
#[must_use]
pub fn house_accept_to_cargo(idx: u8) -> Option<CargoType> {
    match idx {
        0 => Some(CargoType::Passengers),
        1 => Some(CargoType::Mail),
        // Goods y Food (3) → Goods (proxy hasta existir cargo dedicado).
        2 | 3 => Some(CargoType::Goods),
        // Water → Oil (proxy trópico).
        4 => Some(CargoType::Oil),
        _ => None,
    }
}

/// Aporta aceptación de una casa a contadores por cargo (`AddAcceptedCargo_Town`).
pub fn add_accepted_cargo_of_house(house_id: u16, amounts: &mut [u32; 5]) {
    let Some(hs) = HouseSpec::get(house_id) else {
        return;
    };
    for i in 0..3 {
        let cargo_idx = hs.accepts_cargo[i];
        let amt = u32::from(hs.cargo_acceptance[i]);
        if amt == 0 {
            continue;
        }
        let slot = match cargo_idx {
            0 => 0,     // passengers
            1 => 1,     // mail
            2 | 3 => 2, // goods / food
            4 => 3,     // water → oil slot en coverage
            _ => continue,
        };
        amounts[slot] = amounts[slot].saturating_add(amt);
    }
}

/// Elige un `HouseID` ponderado por zona/clima/año (solo pool vanilla).
#[must_use]
pub fn pick_town_house_id(
    town: &Town,
    zone: HouseZone,
    climate: Climate,
    tile_height: u8,
    calendar_year: u32,
    rng_value: u32,
) -> Option<u16> {
    pick_town_house_id_with_catalog(
        town,
        zone,
        climate,
        tile_height,
        calendar_year,
        rng_value,
        &[],
        &[],
    )
}

/// Pool vanilla + `NewGRF` (1×1 y norte multitile) con filtros clima/zona/año.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn pick_town_house_id_with_catalog(
    town: &Town,
    zone: HouseZone,
    climate: Climate,
    tile_height: u8,
    calendar_year: u32,
    rng_value: u32,
    catalog: &[HouseSpecDef],
    overrides: &[u16],
) -> Option<u16> {
    let climate_mask = climate_zone_mask(climate, tile_height);
    let zone_mask = (1u16 << (zone as u8)) | climate_mask;

    let mut probs: Vec<(u16, u32)> = Vec::new();
    let mut probability_max = 0_u32;

    for (id, &is_1x1) in HOUSE_SIZE_1X1.iter().enumerate() {
        if !is_1x1 {
            continue;
        }
        let house_id = u16::try_from(id).unwrap_or(0);
        if overrides.get(id).is_some_and(|&ovr| ovr != INVALID_HOUSE) {
            continue;
        }
        let Some(hs) = HouseSpec::get(house_id) else {
            continue;
        };
        if !hs.matches_zones(zone_mask) {
            continue;
        }
        if calendar_year < hs.min_year || calendar_year > hs.max_year {
            continue;
        }
        if hs.is_church() && town.has_church {
            continue;
        }
        if hs.is_stadium() && town.has_stadium {
            continue;
        }
        if hs.population == 0 && !hs.is_church() {
            continue;
        }
        let p = u32::from(hs.probability.max(1));
        probability_max = probability_max.saturating_add(p);
        probs.push((hs.id, p));
    }

    for def in catalog {
        if !def.from_newgrf {
            continue;
        }
        // Pool: 1×1 o tesela norte multitile (tiles adicionales no llevan flag de tamaño).
        if !def.is_size_1x1() && !def.is_multitile_north() {
            continue;
        }
        if !def.matches_zones(zone_mask) {
            continue;
        }
        if calendar_year < def.min_year || calendar_year > def.max_year {
            continue;
        }
        if def.is_church() && town.has_church {
            continue;
        }
        if def.is_stadium() && town.has_stadium {
            continue;
        }
        if def.population == 0 && !def.is_church() {
            continue;
        }
        let p = u32::from(def.probability.max(1));
        probability_max = probability_max.saturating_add(p);
        probs.push((def.id, p));
    }

    if probability_max == 0 || probs.is_empty() {
        return None;
    }

    let mut r = rng_value % probability_max;
    for (id, p) in probs {
        if p > r {
            return Some(id);
        }
        r -= p;
    }
    None
}

/// Distancia al cuadrado entre teselas (`DistanceSquare`).
#[must_use]
pub fn distance_square(a: TileCoord, b: TileCoord) -> u32 {
    let dx = a.x.abs_diff(b.x);
    let dy = a.y.abs_diff(b.y);
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
}

/// Zona urbana del tile respecto al pueblo (`GetTownRadiusGroup`).
#[must_use]
pub fn get_town_radius_group(town: &Town, tile: TileCoord) -> HouseZone {
    let dist = distance_square(tile, town.pos);
    if town.fund_buildings_months != 0 && dist <= 25 {
        return HouseZone::TownCentre;
    }
    let mut smallest = HouseZone::TownEdge;
    for i in 0..NUM_HOUSE_ZONES {
        let radius = town.squared_town_zone_radius[i];
        if radius > 0 && dist < radius {
            // HouseZone valores 0..4 coinciden con el índice de radio.
            if let Some(zone) = HouseZone::from_zone_index(i) {
                smallest = zone;
            }
        }
    }
    smallest
}

/// Iteraciones de `GrowTownAtRoad` según layout y casas.
#[must_use]
pub fn grow_town_at_road_iterations(layout: TownLayout, num_houses: u16) -> i32 {
    let n = i32::from(num_houses);
    match layout {
        TownLayout::BetterRoads => 10 + n * 2 / 9,
        TownLayout::Grid2x2 | TownLayout::Grid3x3 => 10 + n / 9,
        TownLayout::Original | TownLayout::Random => 10 + n * 4 / 9,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::TileCoord;
    use crate::newgrf_sprites::{
        DecodedSprite, TileLayout, TileLayoutSpriteRef, TrainSpriteAssign, TrainSpriteGraphics,
    };
    use crate::town::{Town, update_town_radius};

    #[test]
    fn tall_office_is_temperate_centre_only() {
        let hs = HouseSpec::get(0).unwrap();
        assert_eq!(hs.population, 187);
        assert!(hs.is_size_1x1());
        let centre_temp =
            (1 << HouseZone::TownCentre as u8) | (1 << HouseZone::ClimateTemperate as u8);
        assert!(hs.matches_zones(centre_temp));
        let edge_temp = (1 << HouseZone::TownEdge as u8) | (1 << HouseZone::ClimateTemperate as u8);
        assert!(!hs.matches_zones(edge_temp));
    }

    #[test]
    fn church_flag_and_unique() {
        let hs = HouseSpec::get(3).unwrap();
        assert!(hs.is_church());
        assert_eq!(hs.population, 5);
    }

    #[test]
    fn pick_respects_year_and_zone() {
        let mut town = Town {
            pos: TileCoord::new(10, 10),
            num_houses: 48,
            fund_buildings_months: 0,
            ..Default::default()
        };
        update_town_radius(&mut town);
        // Con 48 casas el radio centre es 9; dist 0 → TownCentre.
        let zone = get_town_radius_group(&town, TileCoord::new(10, 10));
        assert_eq!(zone, HouseZone::TownCentre);
        let id = pick_town_house_id(&town, zone, Climate::Temperate, 1, 1980, 42).unwrap();
        let hs = HouseSpec::get(id).unwrap();
        assert!(hs.is_size_1x1());
        assert!(hs.min_year <= 1980 && hs.max_year >= 1980);
    }

    #[test]
    fn acceptance_sums_goods_from_office() {
        let mut amounts = [0u32; 5];
        add_accepted_cargo_of_house(0, &mut amounts);
        assert_eq!(amounts[0], 8); // passengers
        assert_eq!(amounts[1], 3); // mail
        assert_eq!(amounts[2], 4); // goods
    }

    #[test]
    fn multitile_footprint_2x2_four_offsets() {
        let offs = house_footprint_offsets(BUILDING_FLAG_SIZE_2X2);
        assert_eq!(offs, vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
        assert_eq!(house_footprint_offsets(BUILDING_FLAG_SIZE_2X1).len(), 2);
        assert_eq!(house_footprint_offsets(BUILDING_FLAG_SIZE_1X1).len(), 1);
    }

    #[test]
    fn resolve_draw_uses_subst_without_views() {
        let catalog = vec![HouseSpecDef {
            id: NEW_HOUSE_OFFSET,
            local_id: 0,
            subst_id: 7,
            building_flags: BUILDING_FLAG_SIZE_1X1,
            min_year: 0,
            max_year: HOUSE_YEAR_MAX,
            population: 10,
            mail_generation: 1,
            availability: DEFAULT_HOUSE_AVAILABILITY,
            probability: DEFAULT_HOUSE_PROBABILITY,
            override_id: None,
            callback_mask: 0,
            name: "H".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: None,
        }];
        assert_eq!(resolve_house_draw_id(NEW_HOUSE_OFFSET, &catalog), 7);
        assert_eq!(resolve_house_draw_id(200, &[]), 200 % NEW_HOUSE_OFFSET);
    }

    #[test]
    fn house_action2_context_matches_persisted_scope_fields() {
        let mut tile = Tile::completed_house(7, 19, 0);
        tile.m1 = 0xAB; // GetHouseRandomBits
        tile.m3 = 0x95; // completed + waiting randomisation triggers
        tile.m3hi = 4; // animation frame in the local map representation
        let ctx = action2_eval_ctx_for_house_tile(tile, 5, 2, Climate::Temperate);

        // stage 3 + TileHash2Bit(5,2)=2 in bits 2..3.
        assert_eq!(ctx.vars.get(&0x40), Some(&11));
        assert_eq!(ctx.vars.get(&0x41), Some(&19));
        assert_eq!(ctx.vars.get(&0x43), Some(&0));
        assert_eq!(ctx.vars.get(&0x46), Some(&4));
        assert_eq!(ctx.vars.get(&0x47), Some(&((2 << 16) | 5)));
        assert_eq!(ctx.random_bits, 0xAB);
        assert_eq!(ctx.vars.get(&0x5F), Some(&((0xAB << 8) | 0x15)));
        assert!(!ctx.vars.contains_key(&0x42));
        assert!(!ctx.vars.contains_key(&0x44));
    }

    #[test]
    fn runtime_tile_layout_resolves_ground_and_sequence() {
        let sprite = DecodedSprite {
            width: 2,
            height: 2,
            x_offs: -1,
            y_offs: 3,
            rgba: [96, 128, 160, 255].repeat(4),
            mask: Vec::new(),
        };
        let mut runtime = TrainSpriteGraphics {
            sets: vec![vec![sprite.clone()], vec![sprite.clone()]],
            assigns: vec![TrainSpriteAssign {
                local_id: 12,
                set_id: 5,
            }],
            ..Default::default()
        };
        runtime.tile_layouts.insert(
            5,
            TileLayout {
                ground: TileLayoutSpriteRef {
                    action1_set: Some(0),
                    ..Default::default()
                },
                sequence: vec![TileLayoutSpriteRef {
                    action1_set: Some(1),
                    origin: [4, 5, 6],
                    extent: [8, 8, 16],
                    ..Default::default()
                }],
            },
        );
        let def = HouseSpecDef {
            id: NEW_HOUSE_OFFSET,
            local_id: 12,
            subst_id: 0,
            building_flags: BUILDING_FLAG_SIZE_1X1,
            min_year: 0,
            max_year: HOUSE_YEAR_MAX,
            population: 1,
            mail_generation: 1,
            availability: DEFAULT_HOUSE_AVAILABILITY,
            probability: DEFAULT_HOUSE_PROBABILITY,
            override_id: None,
            callback_mask: 0,
            name: "layout".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_views: vec![sprite.clone()],
            newgrf_local_id: 12,
            newgrf_runtime: Some(Box::new(runtime)),
        };
        let mut ctx = crate::newgrf_sprites::Action2EvalCtx::default();
        let Some(layout) = def.newgrf_tile_layout_runtime(2, &mut ctx) else {
            panic!("house TileSeq");
        };
        assert!(layout.complete);
        assert!(layout.ground.is_some());
        assert_eq!(layout.sequence[0].origin, [4, 5, 6]);
    }
}
