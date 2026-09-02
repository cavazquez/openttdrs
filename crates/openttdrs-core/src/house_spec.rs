//! Specs de casas vanilla (`HouseSpec` / `_original_house_specs`) y `NewGRF` (`HouseSpecDef`).
//!
//! Datos generados en el módulo privado `crate::sav::house_population_generated`; aquí viven
//! las consultas runtime para zonas, años, pesos y aceptación (P3.5–P3.7). Feature Action0 `0x07`.

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::map::{Map, Tile, TileCoord, TileKind, tile_slope_and_z, water_class};
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
/// Bit `HouseCallbackMask::DrawFoundations`: consulta CB `0x150` al dibujar
/// una casa sobre una pendiente.
pub const HOUSE_CALLBACK_DRAW_FOUNDATIONS_MASK: u16 = 1 << 11;

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

/// Conteos de edificios que consume `HouseScopeResolver`.
///
/// `OpenTTD` mantiene estos contadores en un cache global y otro por pueblo.
/// El renderer consulta muchas casas durante un mismo pase, por lo que no
/// debe recorrer el mapa para cada tesela: esta instantánea se construye una
/// vez por pase y se reutiliza en todas las evaluaciones Action2.
#[derive(Debug, Clone, Default)]
pub struct HouseScopeCounts {
    map_by_id: Vec<u32>,
    town_by_id: std::collections::HashMap<(u32, u16), u32>,
}

impl HouseScopeCounts {
    /// Construye los conteos a partir de `MAP8` y `MAP2`.
    ///
    /// `MAP2` es `TownID` para `MP_HOUSE`. En mapas legacy donde ese id no
    /// identifica un pueblo cargado, se conserva el fallback de `OpenTTD` que
    /// usa el pueblo más cercano para resolver el scope.
    #[must_use]
    pub fn from_map(map: &Map, towns: &[Town]) -> Self {
        let mut counts = Self {
            map_by_id: vec![0; 1 << 12],
            town_by_id: std::collections::HashMap::new(),
        };
        let (width, height) = map.dimensions();
        for y in 0..height {
            for x in 0..width {
                let coord = TileCoord::new(x.cast_signed(), y.cast_signed());
                let Some(tile) = map.get(coord) else {
                    continue;
                };
                if tile.kind != TileKind::House {
                    continue;
                }
                let house_id = tile.m8 & 0x0FFF;
                if let Some(slot) = counts.map_by_id.get_mut(usize::from(house_id)) {
                    *slot = slot.saturating_add(1);
                }
                if let Some(town_id) = house_town_id(tile, coord, towns) {
                    let entry = counts.town_by_id.entry((town_id, house_id)).or_default();
                    *entry = entry.saturating_add(1);
                }
            }
        }
        counts
    }

    /// Número de teselas con el `HouseID` indicado en todo el mapa.
    #[must_use]
    pub fn map_count(&self, house_id: u16) -> u32 {
        self.map_by_id
            .get(usize::from(house_id & 0x0FFF))
            .copied()
            .unwrap_or(0)
    }

    /// Número de teselas con el `HouseID` indicado dentro de un pueblo.
    #[must_use]
    pub fn town_count(&self, town_id: u32, house_id: u16) -> u32 {
        self.town_by_id
            .get(&(town_id, house_id & 0x0FFF))
            .copied()
            .unwrap_or(0)
    }
}

impl HouseSpecDef {
    /// ¿El GRF declaró CB `0x17` para autorizar la construcción de la casa?
    #[must_use]
    pub const fn has_construction_callback(&self) -> bool {
        self.callback_mask & HOUSE_CALLBACK_ALLOW_CONSTRUCTION_MASK != 0
    }

    /// ¿El GRF decide si se dibuja la fundación nivelada (`CB 0x150`)?
    #[must_use]
    pub const fn has_draw_foundations_callback(&self) -> bool {
        self.callback_mask & HOUSE_CALLBACK_DRAW_FOUNDATIONS_MASK != 0
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
/// Este wrapper conserva el contrato histórico para callers que todavía no
/// tienen el vector de pueblos: las variables dependientes del pueblo siguen
/// ausentes en ese caso. El renderer usa
/// [`action2_eval_ctx_for_house_tile_with_towns`] para materializar la zona
/// urbana real.
#[must_use]
pub fn action2_eval_ctx_for_house_tile(
    tile: Tile,
    tx: i32,
    ty: i32,
    climate: Climate,
) -> crate::newgrf_sprites::Action2EvalCtx {
    let mut ctx = action2_eval_ctx_for_house_tile_with_towns(tile, tx, ty, climate, &[]);
    // La API legacy no podía resolver el pueblo asociado y sus tests/callers
    // distinguen una variable no disponible de `TownEdge` (valor cero).
    ctx.vars.remove(&0x42);
    ctx
}

/// Construye el contexto Action2 de una casa con los pueblos del mapa.
///
/// `HouseScopeResolver::GetVariable(0x42)` devuelve la zona del pueblo
/// asociado a la casa. Los mapas importados conservan el `TownID` en `MAP2`;
/// cuando ese dato no está disponible (mapas procedurales antiguos) se usa el
/// pueblo más cercano, la misma aproximación que emplean otros scopes hasta
/// completar la asociación estructural.
#[must_use]
pub fn action2_eval_ctx_for_house_tile_with_towns(
    tile: Tile,
    tx: i32,
    ty: i32,
    climate: Climate,
    towns: &[Town],
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
    let coord = TileCoord::new(tx, ty);
    let town_zone = house_town_id(tile, coord, towns)
        .and_then(|town_id| towns.iter().find(|town| town.id == town_id))
        .map_or(HouseZone::TownEdge, |town| {
            get_town_radius_group(town, coord)
        });
    ctx.vars.insert(0x42, u32::from(town_zone as u8));
    // `HouseScopeResolver::GetVariable(0x45)`: true only while the world is
    // being generated. Runtime rendering and town expansion happen after the
    // generation pass, so the normal map context is zero.
    ctx.vars.insert(0x45, 0);
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

/// Construye un contexto de casa con conteos globales y teselas vecinas.
///
/// `neighbor_params` debe contener los pares `(variable, parámetro)` que el
/// grafo Action2 solicita. Las variables `0x60`/`0x61` son conteos
/// parametrizados; `0x62` devuelve la información de terreno de la tesela
/// vecina y `0x63` su frame de animación si es una casa.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_house_tile_with_counts(
    map: &Map,
    tile: Tile,
    tx: i32,
    ty: i32,
    climate: Climate,
    towns: &[Town],
    house_catalog: &[HouseSpecDef],
    counts: &HouseScopeCounts,
    neighbor_params: &[(u8, u8)],
) -> crate::newgrf_sprites::Action2EvalCtx {
    let mut ctx = action2_eval_ctx_for_house_tile_with_towns(tile, tx, ty, climate, towns);
    let coord = TileCoord::new(tx, ty);
    let house_id = tile.m8 & 0x0FFF;
    let town_id = house_town_id(tile, coord, towns);
    let map_count = counts.map_count(house_id).min(u32::from(u8::MAX));
    let town_count = town_id
        .map_or(0, |id| counts.town_count(id, house_id))
        .min(u32::from(u8::MAX));
    // `GetNumHouses`: class counts occupy the high bytes. Class IDs are not
    // represented by the current catalog yet, so leave those two bytes zero
    // while preserving the exact map/town ID count layout.
    ctx.vars.insert(0x44, (map_count << 8) | town_count);

    let current_def = house_catalog.iter().find(|def| def.id == house_id);
    // HouseResolverObject expone el pueblo como scope parent. En particular,
    // `7C` debe seleccionar la fila PSA por GRFID; sin este write-through una
    // casa NewGRF que consulta estado persistente volvía siempre cero después
    // de cargar un SAV.
    if let (Some(town_id), Some(def)) = (town_id, current_def)
        && let Some(town) = towns.iter().find(|town| town.id == town_id)
    {
        town.copy_newgrf_parent_scope(def.grfid, &mut ctx);
    }
    for &(variable, parameter) in neighbor_params {
        match variable {
            0x60 => {
                let value = if u16::from(parameter) < NEW_HOUSE_OFFSET {
                    counts.map_count(u16::from(parameter))
                } else {
                    0
                };
                // The town-specific low byte is selected by the current
                // house's associated town, matching `GetNumHouses`.
                let town_value =
                    town_id.map_or(0, |id| counts.town_count(id, u16::from(parameter)));
                ctx.parameterized_vars.insert(
                    (variable, parameter),
                    (value.min(u32::from(u8::MAX)) << 8) | town_value.min(u32::from(u8::MAX)),
                );
            }
            0x61 => {
                let target = current_def.filter(|def| def.from_newgrf).and_then(|def| {
                    house_catalog
                        .iter()
                        .find(|candidate| {
                            candidate.from_newgrf
                                && candidate.grfid == def.grfid
                                && candidate.local_id == parameter
                        })
                        .map(|candidate| candidate.id)
                });
                let value = target.map_or(0, |id| counts.map_count(id).min(u32::from(u8::MAX)));
                let town_value = target
                    .zip(town_id)
                    .map_or(0, |(id, town)| counts.town_count(town, id))
                    .min(u32::from(u8::MAX));
                ctx.parameterized_vars
                    .insert((variable, parameter), (value << 8) | town_value);
            }
            0x62 | 0x63 => {
                let nearby = nearby_house_coord(map, coord, parameter);
                let value = if variable == 0x62 {
                    nearby_house_tile_information(map, nearby, climate)
                } else {
                    map.get(nearby)
                        .filter(|candidate| candidate.kind == TileKind::House)
                        .map_or(0, |candidate| u32::from(candidate.m3hi))
                };
                ctx.parameterized_vars.insert((variable, parameter), value);
            }
            _ => {}
        }
    }
    ctx
}

/// Variante cómoda que calcula los conteos para callers pequeños/tests.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn action2_eval_ctx_for_house_tile_with_map(
    map: &Map,
    tile: Tile,
    tx: i32,
    ty: i32,
    climate: Climate,
    towns: &[Town],
    house_catalog: &[HouseSpecDef],
    neighbor_params: &[(u8, u8)],
) -> crate::newgrf_sprites::Action2EvalCtx {
    let counts = HouseScopeCounts::from_map(map, towns);
    action2_eval_ctx_for_house_tile_with_counts(
        map,
        tile,
        tx,
        ty,
        climate,
        towns,
        house_catalog,
        &counts,
        neighbor_params,
    )
}

fn house_town_id(tile: Tile, coord: TileCoord, towns: &[Town]) -> Option<u32> {
    let persisted = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
    towns
        .iter()
        .find(|town| town.id == persisted)
        .map(|town| town.id)
        .or_else(|| {
            towns
                .iter()
                .min_by_key(|town| distance_square(town.pos, coord))
                .map(|town| town.id)
        })
}

fn nearby_house_coord(map: &Map, base: TileCoord, parameter: u8) -> TileCoord {
    let (width, height) = map.dimensions();
    let (Ok(width), Ok(height)) = (i32::try_from(width), i32::try_from(height)) else {
        return base;
    };
    if width == 0 || height == 0 {
        return base;
    }
    let signed_nibble = |value: u8| {
        let value = i32::from(value & 0x0F);
        if value >= 8 { value - 16 } else { value }
    };
    TileCoord::new(
        base.x
            .saturating_add(signed_nibble(parameter))
            .rem_euclid(width),
        base.y
            .saturating_add(signed_nibble(parameter >> 4))
            .rem_euclid(height),
    )
}

fn nearby_house_tile_information(map: &Map, coord: TileCoord, climate: Climate) -> u32 {
    let Some(tile) = map.get(coord) else {
        return 0;
    };
    let (tileh, z) = tile_slope_and_z(map, coord).unwrap_or((0, tile.height));
    let terrain = if climate.uses_desert_patches() && tile.m7 & 0x20 != 0 {
        1
    } else if climate.uses_snow_ground() || tile.m7 & 0x20 != 0 {
        4
    } else {
        0
    };
    let water_info = water_class(tile).map_or(0, |water| (water as u8 + 1) & 3);
    let is_water = u8::from(tile.kind == TileKind::Water);
    let terrain_info = (water_info << 5) | (terrain << 2) | (is_water << 1);
    let tile_type = if tile.ottd_type_nibble() != 0 || tile.kind == TileKind::Grass {
        tile.ottd_type_nibble()
    } else {
        match tile.kind {
            TileKind::Water => 6,
            TileKind::Forest => 4,
            TileKind::Road | TileKind::RoadDepot | TileKind::RoadTunnel | TileKind::RoadBridge => 2,
            TileKind::Rail | TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge => 1,
            TileKind::House => 3,
            TileKind::Station => 5,
            TileKind::Industry => 8,
            TileKind::Void => 7,
            TileKind::ShipDepot
            | TileKind::Airport
            | TileKind::CoalField
            | TileKind::Unknown(_) => tile.ottd_type_nibble(),
            TileKind::Grass => 0,
        }
    };
    (u32::from(tile_type) << 24)
        | (u32::from(z) << 16)
        | (u32::from(terrain_info) << 8)
        | u32::from(tileh)
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

/// Offsets del footprint multitile a partir de la tesela base.
///
/// El orden no es sólo geométrico: `MakeTownHouse` incrementa el `HouseID`
/// como base, `+Y`, `+X`, `+X+Y`. Mantenerlo aquí hace que la ruta runtime
/// de casas terminadas coincida con el escritor nativo de generación.
#[must_use]
pub fn house_footprint_offsets(building_flags: u8) -> Vec<(i32, i32)> {
    if building_flags & BUILDING_FLAG_SIZE_2X2 != 0 {
        return vec![(0, 0), (0, 1), (1, 0), (1, 1)];
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
    climate_zone_mask_at_snow_line(climate, tile_height, DEF_SNOW_LINE_HEIGHT)
}

/// Máscara de clima usando la línea de nieve efectiva del mapa.
///
/// `OpenTTD` recalcula `game_creation.snow_line_height` a partir de la
/// cobertura ártica antes de `GenerateTowns`. La variante histórica de
/// [`climate_zone_mask`] conserva el valor por defecto para los consumidores
/// que no tienen contexto de partida; la generación de pueblos debe usar esta
/// función con la línea persistida en `GameState`.
#[must_use]
pub fn climate_zone_mask_at_snow_line(
    climate: Climate,
    tile_height: u8,
    snow_line_height: u8,
) -> u16 {
    match climate {
        Climate::Temperate => 1 << (HouseZone::ClimateTemperate as u8),
        Climate::SubArctic => {
            if i32::from(tile_height) > i32::from(snow_line_height) {
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
    fn arctic_house_zone_uses_effective_snow_line() {
        let above_snow = 1 << HouseZone::ClimateSubarcticAboveSnow as u8;
        let below_snow = 1 << HouseZone::ClimateSubarcticBelowSnow as u8;

        assert_eq!(
            climate_zone_mask_at_snow_line(Climate::SubArctic, 3, 2),
            above_snow
        );
        assert_eq!(
            climate_zone_mask_at_snow_line(Climate::SubArctic, 3, DEF_SNOW_LINE_HEIGHT),
            below_snow
        );
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
        assert_eq!(offs, vec![(0, 0), (0, 1), (1, 0), (1, 1)]);
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
    fn house_draw_foundations_callback_uses_upstream_mask() {
        let mut def = HouseSpecDef {
            id: NEW_HOUSE_OFFSET,
            local_id: 0,
            subst_id: 0,
            building_flags: BUILDING_FLAG_SIZE_1X1,
            min_year: 0,
            max_year: HOUSE_YEAR_MAX,
            population: 1,
            mail_generation: 0,
            availability: DEFAULT_HOUSE_AVAILABILITY,
            probability: DEFAULT_HOUSE_PROBABILITY,
            override_id: None,
            callback_mask: 0,
            name: "foundation-callback".into(),
            from_newgrf: true,
            grfid: 1,
            newgrf_views: Vec::new(),
            newgrf_local_id: 0,
            newgrf_runtime: None,
        };
        assert!(!def.has_draw_foundations_callback());
        def.callback_mask = HOUSE_CALLBACK_DRAW_FOUNDATIONS_MASK;
        assert!(def.has_draw_foundations_callback());
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
    fn house_action2_context_exposes_nearest_town_zone() {
        let mut tile = Tile::completed_house(7, 19, 0);
        let mut town = Town {
            id: 7,
            pos: TileCoord::new(5, 2),
            num_houses: 48,
            ..Default::default()
        };
        update_town_radius(&mut town);
        tile.m2 = 7;

        let ctx = action2_eval_ctx_for_house_tile_with_towns(
            tile,
            5,
            2,
            Climate::Temperate,
            std::slice::from_ref(&town),
        );

        assert_eq!(ctx.vars.get(&0x42), Some(&(HouseZone::TownCentre as u32)));
    }

    #[test]
    fn house_action2_map_context_exposes_counts_and_neighbours() {
        let mut map = Map::new_flat(4, 1, 0);
        let first = TileCoord::new(0, 0);
        let second = TileCoord::new(1, 0);
        map.set_completed_house(first, 7, 0).unwrap();
        map.set_house_town_id(first, 3).unwrap();
        map.set_completed_house(second, 7, 0).unwrap();
        map.set_house_town_id(second, 3).unwrap();
        let mut animated = map.get(second).unwrap();
        animated.m3hi = 9;
        map.set_tile(second, animated).unwrap();
        crate::map::make_water_tile(&mut map, TileCoord::new(3, 0), crate::map::WaterClass::Sea)
            .unwrap();
        let town = Town {
            id: 3,
            pos: first,
            num_houses: 2,
            ..Default::default()
        };
        let ctx = action2_eval_ctx_for_house_tile_with_map(
            &map,
            map.get(first).unwrap(),
            first.x,
            first.y,
            Climate::Temperate,
            std::slice::from_ref(&town),
            &[],
            &[(0x60, 7), (0x62, 0x0F), (0x63, 1)],
        );
        assert_eq!(ctx.vars.get(&0x44), Some(&0x0202));
        assert_eq!(ctx.parameterized_vars.get(&(0x60, 7)), Some(&0x0202));
        assert_eq!(ctx.parameterized_vars.get(&(0x63, 1)), Some(&9));
        let nearby_water = *ctx.parameterized_vars.get(&(0x62, 0x0F)).unwrap();
        assert_eq!(nearby_water >> 24, 6);
        assert_eq!(nearby_water & 0xFF, 0);
    }

    #[test]
    fn house_action2_map_context_exposes_town_persistent_scope() {
        let mut map = Map::new_flat(2, 1, 0);
        let coord = TileCoord::new(0, 0);
        map.set_completed_house(coord, 7, 0).unwrap();
        map.set_house_town_id(coord, 3).unwrap();
        let mut town = Town {
            id: 3,
            pos: coord,
            ..Default::default()
        };
        town.newgrf_persistent_regs
            .entry(0x1122_3344)
            .or_default()
            .insert(7, 0xCAFE_BABE);
        let def = HouseSpecDef {
            id: 7,
            local_id: 2,
            subst_id: 0,
            building_flags: BUILDING_FLAG_SIZE_1X1,
            min_year: 0,
            max_year: HOUSE_YEAR_MAX,
            population: 1,
            mail_generation: 0,
            availability: DEFAULT_HOUSE_AVAILABILITY,
            probability: DEFAULT_HOUSE_PROBABILITY,
            override_id: None,
            callback_mask: 0,
            name: "town-psa".into(),
            from_newgrf: true,
            grfid: 0x1122_3344,
            newgrf_views: Vec::new(),
            newgrf_local_id: 2,
            newgrf_runtime: None,
        };
        let ctx = action2_eval_ctx_for_house_tile_with_map(
            &map,
            map.get(coord).unwrap(),
            coord.x,
            coord.y,
            Climate::Temperate,
            std::slice::from_ref(&town),
            std::slice::from_ref(&def),
            &[],
        );
        assert_eq!(ctx.parent_persistent_registers.get(&7), Some(&0xCAFE_BABE));
        assert_eq!(ctx.parent_vars.get(&0x41), Some(&3));
        assert_eq!(ctx.parent_vars.get(&0x82), Some(&0));
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
