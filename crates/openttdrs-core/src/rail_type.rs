//! Tipos de vía (`RailType` / `RailTypeInfo` de `OpenTTD`).
//!
//! Persistidos en `Tile.m8` bits 0–5 (`GetRailType` / `SetRailType` en `rail_map.h`).
//! Fase 5: normal + eléctrico. Fase 6: monorail + maglev.
//!
//! El tranvía en `OpenTTD` es `RoadType` (no `RailType`); queda fuera de este módulo.

use serde::{Deserialize, Serialize};

use crate::engine::{
    ENGINE_TRAIN_ASIASTAR, ENGINE_TRAIN_LEV1, ENGINE_TRAIN_SH_30, ENGINE_TRAIN_SH_40,
    ENGINE_TRAIN_TIM, ENGINE_TRAIN_X2001, EngineDef,
};
use crate::map::Tile;

/// Identificador de tipo de vía (valores alineados con `RailType` upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum RailType {
    #[default]
    Rail = 0,
    Electric = 1,
    Monorail = 2,
    Maglev = 3,
}

impl RailType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x3F {
            1 => Self::Electric,
            2 => Self::Monorail,
            3 => Self::Maglev,
            _ => Self::Rail,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rail => "Normal",
            Self::Electric => "Eléctrica",
            Self::Monorail => "Monorail",
            Self::Maglev => "Maglev",
        }
    }

    /// ¿La vía lleva catenaria (render / compat)?
    #[must_use]
    pub const fn has_catenary(self) -> bool {
        matches!(self, Self::Electric)
    }

    /// Siguiente tipo en el ciclo de conversión / construcción.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Rail => Self::Electric,
            Self::Electric => Self::Monorail,
            Self::Monorail => Self::Maglev,
            Self::Maglev => Self::Rail,
        }
    }

    /// Índice para tablas de movimiento (`ACCEL_SLOWDOWN[3]`): 0=rail/el, 1=mono, 2=maglev.
    #[must_use]
    pub const fn accel_table_index(self) -> usize {
        match self {
            Self::Rail | Self::Electric => 0,
            Self::Monorail => 1,
            Self::Maglev => 2,
        }
    }

    /// Límite de velocidad del tipo de vía (`RailTypeInfo::max_speed`).
    ///
    /// `0` = sin límite (vanilla `OpenTTD`: rail/electric/mono/maglev = 0).
    #[must_use]
    pub const fn max_speed(self) -> u16 {
        // Tabla `_original_railtypes`: todos 0. NewGRF puede imponer techos > 0.
        let _ = self;
        0
    }
}

/// Selector Action3 `RailSpriteType::Signals` de `OpenTTD`.
pub const RAIL_SPRITE_TYPE_SIGNALS: u8 = 11;
/// Selector Action3 `RailSpriteType::TrackOverlay` (guías / catenaria overlay).
pub const RAIL_SPRITE_TYPE_TRACK_OVERLAY: u8 = 1;
/// Selector Action3 `RailSpriteType::Underlay` / ground.
pub const RAIL_SPRITE_TYPE_UNDERLAY: u8 = 0;

/// Props Action0 runtime por `RailType` vanilla (reconstruidas desde el stack).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RailTypeRuntimeProps {
    pub max_speed: u16,
    pub cost_multiplier: u16,
    pub maintenance_multiplier: u16,
    pub flags: u8,
    pub curve_speed: u8,
    pub introduction_date: u32,
    /// Bitmask compatible; `0` = reglas vanilla.
    pub compatible_mask: u8,
    /// Bitmask powered; `0` = reglas vanilla.
    pub powered_mask: u8,
}

impl RailTypeRuntimeProps {
    #[must_use]
    pub const fn defaults() -> [Self; 4] {
        [Self {
            max_speed: 0,
            cost_multiplier: 0,
            maintenance_multiplier: 0,
            flags: 0,
            curve_speed: 0,
            introduction_date: 0,
            compatible_mask: 0,
            powered_mask: 0,
        }; 4]
    }
}

/// ¿Compatibles según máscaras NewGRF (si hay) o reglas vanilla?
#[must_use]
pub fn rail_types_compatible_with_props(
    a: RailType,
    b: RailType,
    props: &[RailTypeRuntimeProps; 4],
) -> bool {
    let ia = usize::from(a.as_u8());
    let ib = usize::from(b.as_u8());
    let pa = props.get(ia).copied().unwrap_or_default();
    let pb = props.get(ib).copied().unwrap_or_default();
    if pa.compatible_mask != 0 {
        return a == b || railtypes_mask_contains(pa.compatible_mask, b);
    }
    if pb.compatible_mask != 0 {
        return a == b || railtypes_mask_contains(pb.compatible_mask, a);
    }
    rail_types_compatible(a, b)
}

/// Máscara powered con override NewGRF.
#[must_use]
pub fn powered_railtypes_mask_with_props(
    rt: RailType,
    props: &[RailTypeRuntimeProps; 4],
) -> u8 {
    let p = props
        .get(usize::from(rt.as_u8()))
        .copied()
        .unwrap_or_default();
    if p.powered_mask != 0 {
        p.powered_mask
    } else {
        powered_railtypes_mask(rt)
    }
}

/// Coste de construcción factored por `prop 0x13` (`8` = ×1 en OTTD).
#[must_use]
pub fn rail_build_cost_multiplier(props: &RailTypeRuntimeProps) -> u16 {
    if props.cost_multiplier == 0 {
        8
    } else {
        props.cost_multiplier
    }
}

/// Gráfico Action3 de un `RailType` `NewGRF` (señales u otro sprite type).
///
/// Es efímero: se reconstruye desde el stack y nunca se serializa en el save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RailSignalSpriteSpec {
    pub rail_type: RailType,
    pub local_id: u8,
    /// Selector Action3 (`RailSpriteType`).
    pub sprite_type: u8,
    pub grfid: u32,
    pub type_tables: Option<crate::newgrf_type_tables::GrfTypeTranslationTables>,
    pub graphics: crate::newgrf_sprites::TrainSpriteGraphics,
}

impl RailSignalSpriteSpec {
    /// Replica `GetCustomSignalSprite`: resuelve Action2 con param1/param2 y
    /// devuelve el sprite en el offset `image` del `ResultSpriteGroup`.
    pub fn resolve_sprite(
        &self,
        image: u8,
        signal_type: u8,
        variant: u8,
        green: bool,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::DecodedSprite> {
        ctx.vars.insert(0x10, 0); // param1: gui=false
        ctx.vars.insert(
            0x18,
            (u32::from(signal_type) << 16) | (u32::from(variant) << 8) | u32::from(green),
        );
        self.resolve_group(image, ctx)
    }

    /// Resuelve el grupo Action3 del `sprite_type` almacenado (fallback: vacío → `None`).
    pub fn resolve_group(
        &self,
        image: u8,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::DecodedSprite> {
        self.graphics
            .views_for_specific_ctx(self.local_id, self.sprite_type, ctx)?
            .get(usize::from(image))
            .cloned()
    }
}

/// Velocidad máxima efectiva de la tesela para un tren (`GetMaxTrackSpeed`).
///
/// `overrides` indexa por `RailType` vanilla (Action0 prop `0x14`). Devuelve
/// `None` si no hay techo (`0`).
#[must_use]
pub fn rail_type_track_speed_cap(tile: Tile, overrides: &[u16; 4]) -> Option<u16> {
    let rt = rail_type_from_tile(tile);
    let cap = overrides
        .get(usize::from(rt.as_u8()))
        .copied()
        .unwrap_or(0)
        .max(rt.max_speed());
    (cap > 0).then_some(cap)
}

/// Máscara de bits 0–5 de `m8` (tipo de vía).
const RAILTYPE_M8_MASK: u16 = 0x003F;

/// Lee el tipo de vía de una tesela (`GetRailType`).
#[must_use]
pub fn rail_type_from_tile(tile: Tile) -> RailType {
    RailType::from_u8(u8::try_from(tile.m8 & RAILTYPE_M8_MASK).unwrap_or(0))
}

/// Escribe el tipo de vía en `m8` preservando el resto de bits.
#[must_use]
pub fn set_rail_type_on_tile(mut tile: Tile, rail_type: RailType) -> Tile {
    tile.m8 = (tile.m8 & !RAILTYPE_M8_MASK) | u16::from(rail_type.as_u8());
    tile
}

/// Coste de convertir una tesela de vía (`CmdConvertRail` simplificado).
pub const RAIL_CONVERT_COST: i64 = 15;

/// ¿Dos tipos de vía son transitables entre sí para pathfinding?
///
/// Estaciones/depósitos no tienen tipo propio: se aceptan siempre.
/// Vapor/diésel: Rail ↔ Electric. Mono y Maglev son redes aisladas.
#[must_use]
pub fn rail_types_compatible(a: RailType, b: RailType) -> bool {
    if a == b {
        return true;
    }
    matches!(
        (a, b),
        (RailType::Rail, RailType::Electric) | (RailType::Electric, RailType::Rail)
    )
}

/// Bitmask de un `RailType` (bit 0=Rail … 3=Maglev) para `compatible_railtypes`.
#[must_use]
pub const fn rail_type_bit(rt: RailType) -> u8 {
    1u8 << (rt as u8)
}

/// `GetAllPoweredRailTypes` simplificado: vías donde el motor obtiene tracción.
#[must_use]
pub const fn powered_railtypes_mask(rt: RailType) -> u8 {
    match rt {
        // Vapor/diésel: circulan en normal y eléctrica.
        RailType::Rail => rail_type_bit(RailType::Rail) | rail_type_bit(RailType::Electric),
        RailType::Electric => rail_type_bit(RailType::Electric),
        RailType::Monorail => rail_type_bit(RailType::Monorail),
        RailType::Maglev => rail_type_bit(RailType::Maglev),
    }
}

/// ¿El bitmask `compatible_railtypes` incluye este tipo de vía?
#[must_use]
pub const fn railtypes_mask_contains(mask: u8, rt: RailType) -> bool {
    mask & rail_type_bit(rt) != 0
}

/// ¿El motor puede circular / comprarse sobre este tipo de vía?
#[must_use]
pub fn engine_compatible_with_rail(engine: &EngineDef, rail_type: RailType) -> bool {
    if !engine.is_train_engine() && !engine.is_wagon() {
        return true;
    }
    let required = engine
        .required_rail_type
        .map(RailType::from_u8)
        .unwrap_or_else(|| required_rail_type_for_engine(engine.id));
    match required {
        RailType::Rail => matches!(rail_type, RailType::Rail | RailType::Electric),
        other => other == rail_type,
    }
}

/// Motores eléctricos del catálogo temperate (ids 110–113).
#[must_use]
pub const fn engine_requires_electric(engine_id: u16) -> bool {
    matches!(
        engine_id,
        ENGINE_TRAIN_SH_30 | ENGINE_TRAIN_SH_40 | ENGINE_TRAIN_TIM | ENGINE_TRAIN_ASIASTAR
    )
}

#[must_use]
pub const fn engine_requires_monorail(engine_id: u16) -> bool {
    engine_id == ENGINE_TRAIN_X2001
}

#[must_use]
pub const fn engine_requires_maglev(engine_id: u16) -> bool {
    engine_id == ENGINE_TRAIN_LEV1
}

/// Tipo de vía requerido por el motor (para UI / pathfinding / compra).
#[must_use]
pub fn required_rail_type_for_engine(engine_id: u16) -> RailType {
    if engine_requires_maglev(engine_id) {
        RailType::Maglev
    } else if engine_requires_monorail(engine_id) {
        RailType::Monorail
    } else if engine_requires_electric(engine_id) {
        RailType::Electric
    } else {
        RailType::Rail
    }
}

/// ¿Una tesela de vía es usable por un motor con `required`?
///
/// Depósitos/estaciones/túneles/puentes no filtran por tipo (MVP).
#[must_use]
pub fn tile_usable_by_rail_type(tile: Tile, required: RailType) -> bool {
    use crate::map::TileKind;
    match tile.kind {
        TileKind::Rail => {
            let t = rail_type_from_tile(tile);
            t == required || rail_types_compatible(t, required)
        }
        TileKind::RailDepot | TileKind::RailTunnel | TileKind::RailBridge | TileKind::Station => {
            true
        }
        _ => false,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::{ENGINE_TRAIN_KIRBY, engine_by_id};
    use crate::map::{Tile, TileKind};

    fn rail_tile(m8: u16) -> Tile {
        Tile {
            height: 1,
            kind: TileKind::Rail,
            mapt: 0x10,
            m5: 0x01,
            m1: 0,
            m6: 0,
            m8,
            m3: 0,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }

    #[test]
    fn m8_roundtrip_all_rail_types() {
        for rt in [
            RailType::Rail,
            RailType::Electric,
            RailType::Monorail,
            RailType::Maglev,
        ] {
            let t = set_rail_type_on_tile(rail_tile(0), rt);
            assert_eq!(rail_type_from_tile(t), rt);
        }
    }

    #[test]
    fn mono_and_maglev_are_isolated() {
        assert!(!rail_types_compatible(RailType::Monorail, RailType::Rail));
        assert!(!rail_types_compatible(RailType::Maglev, RailType::Electric));
        assert!(rail_types_compatible(RailType::Rail, RailType::Electric));
    }

    #[test]
    fn electric_engine_needs_electric_track() {
        let asia = engine_by_id(ENGINE_TRAIN_ASIASTAR).unwrap();
        assert!(!engine_compatible_with_rail(asia, RailType::Rail));
        assert!(engine_compatible_with_rail(asia, RailType::Electric));
        assert!(!engine_compatible_with_rail(asia, RailType::Monorail));
        let kirby = engine_by_id(ENGINE_TRAIN_KIRBY).unwrap();
        assert!(engine_compatible_with_rail(kirby, RailType::Rail));
        assert!(engine_compatible_with_rail(kirby, RailType::Electric));
        assert!(!engine_compatible_with_rail(kirby, RailType::Maglev));
    }

    #[test]
    fn cycle_visits_all_types() {
        let mut t = RailType::Rail;
        let mut seen = [false; 4];
        for _ in 0..4 {
            seen[t.as_u8() as usize] = true;
            t = t.next();
        }
        assert!(seen.iter().all(|&x| x));
        assert_eq!(t, RailType::Rail);
    }
}
