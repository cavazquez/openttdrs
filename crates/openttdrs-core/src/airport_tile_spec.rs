//! Teselas de aeropuerto `NewGRF` (`AirportTiles`, feature `0x11`).
//!
//! Paridad MVP con `NEW_AIRPORTTILE_OFFSET` / manager de tiles:
//! slots globales ≥74, `subst_id` vanilla y overrides de gfx &lt;74.

use serde::{Deserialize, Serialize};

/// Primera tesela definida por `NewGRF` (`OpenTTD` `NEW_AIRPORTTILE_OFFSET`).
pub const NEW_AIRPORT_TILE_OFFSET: u16 = 74;
/// Total de slots de tesela de aeropuerto (`OpenTTD` `NUM_AIRPORTTILES`).
pub const NUM_AIRPORT_TILES: u16 = 256;
/// Id inválido.
pub const INVALID_AIRPORT_TILE: u16 = NUM_AIRPORT_TILES;

/// Triggers de animación de `AirportTile` (`newgrf/station_type.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AirportAnimationTrigger {
    Built = 0,
    TileLoop = 1,
    NewCargo = 2,
    CargoTaken = 3,
    AcceptanceTick = 4,
    AirplaneTouchdown = 5,
}

impl AirportAnimationTrigger {
    /// Bit correspondiente en la propiedad Action0 `0x11`.
    #[must_use]
    pub const fn mask(self) -> u8 {
        1_u8 << (self as u8)
    }

    /// Ordinal que recibe CB `0x152` en el byte bajo de `var 18`.
    #[must_use]
    pub const fn callback_param(self, extra: u8) -> u32 {
        self as u32 | ((extra as u32) << 8)
    }
}

/// Identificador global de gfx de tesela de aeropuerto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AirportTileGfxId(pub u16);

impl AirportTileGfxId {
    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

/// Spec `NewGRF` de una tesela de aeropuerto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirportTileSpecDef {
    /// Gfx global (≥ [`NEW_AIRPORT_TILE_OFFSET`] si `from_newgrf`).
    pub gfx: AirportTileGfxId,
    /// Fallback vanilla (`subst_id` &lt; 74).
    pub subst_id: u16,
    pub from_newgrf: bool,
    /// Callback mask (`prop 0x0E`): bit 0 = siguiente frame, bit 1 = velocidad.
    #[serde(default)]
    pub callback_mask: u8,
    /// `prop 0x0F`: último frame de animación permitido.
    #[serde(default)]
    pub animation_frames: u8,
    /// `prop 0x0F`: 0 = no loop, 1 = loop, `0xFF` = sin animación.
    #[serde(default = "default_airport_animation_status")]
    pub animation_status: u8,
    /// `prop 0x10`: espera como potencia de dos de ticks.
    #[serde(default = "default_airport_animation_speed")]
    pub animation_speed: u8,
    /// `prop 0x11`: máscara de `AirportAnimationTrigger` para CB `0x152`.
    #[serde(default)]
    pub animation_triggers: u8,
    /// Flags internos de animación; bit 0 pasa random al callback de frame.
    #[serde(default)]
    pub animation_special_flags: u8,
    /// Id local Action0/3 en el GRF.
    #[serde(default, skip)]
    pub newgrf_local_id: u8,
    /// GRFID del set.
    #[serde(default, skip)]
    pub newgrf_grfid: u32,
    #[serde(default, skip)]
    pub newgrf_preview: Option<crate::newgrf_sprites::DecodedSprite>,
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
}

const fn default_airport_animation_status() -> u8 {
    0xFF
}

const fn default_airport_animation_speed() -> u8 {
    2
}

impl AirportTileSpecDef {
    #[must_use]
    pub fn newgrf_view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return self.newgrf_preview.as_ref();
        }
        self.newgrf_views.get(idx % self.newgrf_views.len())
    }

    /// Resuelve la vista `Action1/2/3` con el contexto de la tesela.
    ///
    /// Los aeropuertos pueden seleccionar un grupo distinto según la posición,
    /// el frame y el layout padre. El preview estático sigue siendo el fallback
    /// para catálogos antiguos que no tienen un grafo runtime.
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

    /// Indica si el GRF instaló el callback `CBID_AIRPTILE_ANIMATION_NEXT_FRAME`.
    #[must_use]
    pub const fn has_animation_next_frame_callback(&self) -> bool {
        self.callback_mask & 1 != 0
    }

    /// Indica si el GRF instaló el callback `CBID_AIRPTILE_ANIMATION_SPEED`.
    #[must_use]
    pub const fn has_animation_speed_callback(&self) -> bool {
        self.callback_mask & 2 != 0
    }

    #[must_use]
    pub const fn animation_loops(&self) -> bool {
        self.animation_status == 1
    }

    #[must_use]
    pub const fn animation_random_bits(&self) -> bool {
        self.animation_special_flags & 1 != 0
    }

    #[must_use]
    pub fn has_newgrf_sprites(&self) -> bool {
        !self.newgrf_views.is_empty()
            || self.newgrf_preview.is_some()
            || self.newgrf_runtime.is_some()
    }
}

/// Tabla de overrides vanilla → gfx `NewGRF`.
#[must_use]
pub fn empty_airport_tile_overrides() -> Vec<u16> {
    vec![INVALID_AIRPORT_TILE; NEW_AIRPORT_TILE_OFFSET as usize]
}

/// Traduce gfx limpio aplicando override `NewGRF` (si hay).
#[must_use]
pub fn get_translated_airport_tile_id(clean: u16, overrides: &[u16]) -> u16 {
    if (clean as usize) < overrides.len() {
        let ovr = overrides[clean as usize];
        if ovr != INVALID_AIRPORT_TILE {
            return ovr;
        }
    }
    clean
}

/// Siguiente gfx libre ≥ [`NEW_AIRPORT_TILE_OFFSET`].
#[must_use]
pub fn next_free_airport_tile_gfx_id(catalog: &[AirportTileSpecDef]) -> Option<u16> {
    let mut used: Vec<u16> = catalog.iter().map(|d| d.gfx.as_u16()).collect();
    used.sort_unstable();
    used.dedup();
    let mut candidate = NEW_AIRPORT_TILE_OFFSET;
    for &u in &used {
        if u == candidate {
            candidate = candidate.saturating_add(1);
        } else if u > candidate {
            break;
        }
    }
    (candidate < NUM_AIRPORT_TILES).then_some(candidate)
}

/// Gfx de dibujo: id `NewGRF` si hay Action3; si no, `subst_id` fallback.
#[must_use]
pub fn resolve_airport_tile_draw_gfx(gfx: u16, catalog: &[AirportTileSpecDef]) -> u16 {
    if gfx < NEW_AIRPORT_TILE_OFFSET {
        return gfx;
    }
    let Some(def) = catalog.iter().find(|d| d.gfx.as_u16() == gfx) else {
        return gfx;
    };
    if def.has_newgrf_sprites() {
        gfx
    } else {
        def.subst_id
    }
}

/// Pieza de construcción: siempre `subst_id` vanilla (FTA `NewGRF` fuera de alcance).
#[must_use]
pub fn resolve_airport_tile_piece_gfx(gfx: u16, catalog: &[AirportTileSpecDef]) -> u8 {
    if gfx < NEW_AIRPORT_TILE_OFFSET {
        return u8::try_from(gfx).unwrap_or(0);
    }
    catalog
        .iter()
        .find(|d| d.gfx.as_u16() == gfx)
        .map_or(0, |d| u8::try_from(d.subst_id.min(255)).unwrap_or(0))
}
