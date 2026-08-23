//! Clases y specs de estación ferroviaria (`StationClass` / `StationSpec` de `OpenTTD`).
//!
//! Catálogo: vanilla (id 0) + `NewGRF` Action0 Stations (ids ≥1).

use serde::{Deserialize, Serialize};

/// Identificador de clase de estación (`StationClassID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct StationClassId(pub u16);

impl StationClassId {
    pub const DEFAULT: Self = Self(0);
    /// Compatibilidad con el enum anterior.
    #[allow(non_upper_case_globals)]
    pub const Default: Self = Self::DEFAULT;

    #[must_use]
    pub const fn from_u16(v: u16) -> Self {
        Self(v)
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self.0 {
            0 => "Por defecto",
            _ => "NewGRF",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self.0 {
            0 => "Dflt",
            _ => "NGRF",
        }
    }
}

/// Identificador de spec dentro del catálogo global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct StationSpecId(pub u16);

impl StationSpecId {
    pub const DEFAULT_RAIL: Self = Self(0);
    /// Compatibilidad con el enum anterior.
    #[allow(non_upper_case_globals)]
    pub const DefaultRail: Self = Self::DEFAULT_RAIL;

    #[must_use]
    pub const fn from_u16(v: u16) -> Self {
        Self(v)
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }
}

/// Metadatos de una clase (`StationClass`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationClassDef {
    pub id: StationClassId,
    pub label: String,
    pub short_label: String,
    pub from_newgrf: bool,
}

/// Spec de estación (`StationSpec` simplificado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationSpecDef {
    pub id: StationSpecId,
    pub class: StationClassId,
    pub label: String,
    pub short_label: String,
    /// Bits 0..=6 = tamaños 1..=7 deshabilitados; bit 7 = >7.
    pub disallowed_platforms: u8,
    /// Bits 0..=6 = longitudes 1..=7 deshabilitadas; bit 7 = >7.
    pub disallowed_lengths: u8,
    /// Máscara de callbacks Action0 propiedad `0x0B`.
    #[serde(default)]
    pub callback_mask: u8,
    /// Action0 `0x13`: flags generales del spec (bit 2 = CB141 recibe random).
    #[serde(default)]
    pub flags: u8,
    /// Action0 `0x16`: último frame de animación.
    #[serde(default = "default_station_animation_status")]
    pub animation_status: u8,
    /// Action0 `0x16`: último frame alcanzable por la animación.
    #[serde(default)]
    pub animation_frames: u8,
    /// Action0 `0x17`: espera `2^speed` ticks entre frames.
    #[serde(default = "default_station_animation_speed")]
    pub animation_speed: u8,
    /// Action0 `0x18`: máscara de `StationAnimationTrigger`.
    #[serde(default)]
    pub animation_triggers: u16,
    pub from_newgrf: bool,
    /// Preview Action1/3 (primera vista); no se serializa en saves.
    #[serde(default, skip)]
    pub newgrf_preview: Option<crate::newgrf_sprites::DecodedSprite>,
    /// Vistas Action1/3 para in-world (MVP: se usa la primera en plano).
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Id local Action3 en el GRF (re-resolver Action2 en runtime).
    #[serde(default, skip)]
    pub newgrf_local_id: u8,
    /// Graphics completas si Action2 var/random requiere runtime.
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
    /// GRFID del `NewGRF` que definió este spec (0 = vanilla).
    #[serde(default, skip)]
    pub newgrf_grfid: u32,
    /// Versión de formato Action8 del GRF dueño. Determina el fallback de la
    /// tabla de traducción de cargos si el GRF no declaró `GlobalVar 0x09`.
    #[serde(default, skip)]
    pub newgrf_grf_version: u8,
    /// Tablas de traducción del GRF para vars Action2 (`42`, etc.).
    #[serde(default, skip)]
    pub newgrf_type_tables: Option<crate::newgrf_type_tables::GrfTypeTranslationTables>,
    /// Layouts custom Action0 prop `0x0E`: clave `(platforms, length)` → tiletypes.
    #[serde(default, skip)]
    pub custom_layouts: std::collections::HashMap<(u8, u8), Vec<u8>>,
}

/// Bit `StationCallbackMask::Avail` de `OpenTTD`: CB `0x13`.
pub const STATION_CALLBACK_AVAILABILITY_MASK: u8 = 1;
/// Bit `StationCallbackMask::DrawTileLayout` de `OpenTTD`: CB `0x14`.
pub const STATION_CALLBACK_DRAW_TILE_LAYOUT_MASK: u8 = 1 << 1;
/// Bit `StationCallbackMask::AnimationNextFrame`: CB `0x141`.
pub const STATION_CALLBACK_ANIMATION_NEXT_FRAME_MASK: u8 = 1 << 2;
/// Bit `StationCallbackMask::AnimationSpeed`: CB `0x142`.
pub const STATION_CALLBACK_ANIMATION_SPEED_MASK: u8 = 1 << 3;
/// Bit `StationCallbackMask::SlopeCheck` de `OpenTTD`: CB `0x149`.
pub const STATION_CALLBACK_SLOPE_CHECK_MASK: u8 = 1 << 4;
/// Flag Action0 `0x13`: CB141 recibe bits aleatorios como `param1`.
pub const STATION_FLAG_CB141_RANDOM_BITS: u8 = 1 << 2;

/// Disparadores de animación de estación / road stop de `OpenTTD`.
///
/// Action0 `0x18` almacena una máscara de estos valores, mientras CB140
/// recibe el ordinal en el byte bajo de `var 18`. Mantener ambas operaciones
/// en el tipo evita confundir `TileLoop = 7` con su máscara `1 << 7`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StationAnimationTrigger {
    Built = 0,
    NewCargo = 1,
    CargoTaken = 2,
    VehicleArrives = 3,
    VehicleDeparts = 4,
    VehicleLoads = 5,
    AcceptanceTick = 6,
    TileLoop = 7,
    PathReservation = 8,
}

impl StationAnimationTrigger {
    /// Bit correspondiente en Action0 `0x18`.
    #[must_use]
    pub const fn mask(self) -> u16 {
        1_u16 << (self as u8)
    }

    /// `param2` de CB140: ordinal del trigger y, si corresponde, cargo local
    /// del GRF en bits 8..15 (`var 18`).
    #[must_use]
    pub const fn callback_param(self, cargo_local_id: Option<u8>) -> u32 {
        let trigger = self as u32;
        match cargo_local_id {
            Some(cargo) => trigger | (cargo as u32) << 8,
            None => trigger,
        }
    }
}

/// Disparadores de re-randomización Action2 de estación / `RoadStop`.
///
/// No comparten los ordinales de [`StationAnimationTrigger`]: por ejemplo,
/// `VehicleArrives` es `2` aquí y `3` en CB140. Separarlos evita usar por
/// error la máscara de animación al evaluar un grupo Action2 random.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StationRandomTrigger {
    NewCargo = 0,
    CargoTaken = 1,
    VehicleArrives = 2,
    VehicleDeparts = 3,
    VehicleLoads = 4,
    PathReservation = 5,
}

impl StationRandomTrigger {
    /// Bit que recibe `ResolverObject::SetWaitingRandomTriggers`.
    #[must_use]
    pub const fn mask(self) -> u8 {
        1_u8 << (self as u8)
    }

    /// Evento Action2 equivalente de un trigger de animación, si existe.
    ///
    /// `Built`, `AcceptanceTick` y `TileLoop` son contratos de CB140, no
    /// eventos de randomización de `RoadStop` en `OpenTTD`.
    #[must_use]
    pub const fn from_animation_trigger(trigger: StationAnimationTrigger) -> Option<Self> {
        match trigger {
            StationAnimationTrigger::NewCargo => Some(Self::NewCargo),
            StationAnimationTrigger::CargoTaken => Some(Self::CargoTaken),
            StationAnimationTrigger::VehicleArrives => Some(Self::VehicleArrives),
            StationAnimationTrigger::VehicleDeparts => Some(Self::VehicleDeparts),
            StationAnimationTrigger::VehicleLoads => Some(Self::VehicleLoads),
            StationAnimationTrigger::PathReservation => Some(Self::PathReservation),
            StationAnimationTrigger::Built
            | StationAnimationTrigger::AcceptanceTick
            | StationAnimationTrigger::TileLoop => None,
        }
    }
}

/// Máscaras Action0 `0x18`, conservadas como API para consumidores existentes.
pub const STATION_ANIMATION_TRIGGER_BUILT: u16 = StationAnimationTrigger::Built.mask();
pub const STATION_ANIMATION_TRIGGER_NEW_CARGO: u16 = StationAnimationTrigger::NewCargo.mask();
pub const STATION_ANIMATION_TRIGGER_CARGO_TAKEN: u16 = StationAnimationTrigger::CargoTaken.mask();
pub const STATION_ANIMATION_TRIGGER_VEHICLE_ARRIVES: u16 =
    StationAnimationTrigger::VehicleArrives.mask();
pub const STATION_ANIMATION_TRIGGER_VEHICLE_DEPARTS: u16 =
    StationAnimationTrigger::VehicleDeparts.mask();
pub const STATION_ANIMATION_TRIGGER_VEHICLE_LOADS: u16 =
    StationAnimationTrigger::VehicleLoads.mask();
pub const STATION_ANIMATION_TRIGGER_ACCEPTANCE_TICK: u16 =
    StationAnimationTrigger::AcceptanceTick.mask();
pub const STATION_ANIMATION_TRIGGER_TILE_LOOP: u16 = StationAnimationTrigger::TileLoop.mask();
pub const STATION_ANIMATION_TRIGGER_PATH_RESERVATION: u16 =
    StationAnimationTrigger::PathReservation.mask();

const fn default_station_animation_status() -> u8 {
    0xFF
}

const fn default_station_animation_speed() -> u8 {
    2
}

impl StationSpecDef {
    /// Id local de un cargo para callbacks de este GRF (`var 18`, bits 8..15).
    #[must_use]
    pub fn newgrf_cargo_local_id(&self, cargo: crate::CargoType, climate: crate::Climate) -> u8 {
        crate::newgrf_type_tables::local_cargo_id(
            self.newgrf_type_tables.as_ref(),
            self.newgrf_grf_version,
            cargo,
            climate,
        )
    }

    /// Preview `NewGRF` si el spec trae sprite Action1/3.
    #[must_use]
    pub fn newgrf_preview_sprite(&self) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        self.newgrf_preview
            .as_ref()
            .or_else(|| self.newgrf_views.first())
    }

    /// Vista in-world (`idx` módulo longitud; MVP suele usar 0).
    #[must_use]
    pub fn newgrf_view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return self.newgrf_preview.as_ref();
        }
        self.newgrf_views.get(idx % self.newgrf_views.len())
    }

    /// Vista re-resolviendo Action2 con contexto (random/variational).
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

    #[must_use]
    pub fn allows_platforms(&self, platforms: u8) -> bool {
        let n = platforms.clamp(1, 7);
        (self.disallowed_platforms & (1 << (n - 1))) == 0
    }

    #[must_use]
    pub fn allows_length(&self, length: u8) -> bool {
        let n = length.clamp(1, 7);
        (self.disallowed_lengths & (1 << (n - 1))) == 0
    }

    /// El spec declaró CB `0x13` de disponibilidad en su máscara Action0.
    #[must_use]
    pub const fn has_availability_callback(&self) -> bool {
        (self.callback_mask & STATION_CALLBACK_AVAILABILITY_MASK) != 0
    }

    /// El spec declaró CB `0x14` para elegir layout al dibujar.
    #[must_use]
    pub const fn has_draw_tile_layout_callback(&self) -> bool {
        (self.callback_mask & STATION_CALLBACK_DRAW_TILE_LAYOUT_MASK) != 0
    }

    /// El spec declaró CB `0x141` para elegir el siguiente frame.
    #[must_use]
    pub const fn has_animation_next_frame_callback(&self) -> bool {
        (self.callback_mask & STATION_CALLBACK_ANIMATION_NEXT_FRAME_MASK) != 0
    }

    /// El spec declaró CB `0x142` para elegir la velocidad de animación.
    #[must_use]
    pub const fn has_animation_speed_callback(&self) -> bool {
        (self.callback_mask & STATION_CALLBACK_ANIMATION_SPEED_MASK) != 0
    }

    /// La secuencia Action0 continúa al llegar al último frame.
    #[must_use]
    pub const fn animation_loops(&self) -> bool {
        self.animation_status == 1
    }

    /// CB141 recibe random bits (`StationSpecFlag::Cb141RandomBits`).
    #[must_use]
    pub const fn animation_next_frame_uses_random_bits(&self) -> bool {
        (self.flags & STATION_FLAG_CB141_RANDOM_BITS) != 0
    }

    /// El spec declaró CB `0x149` de comprobación de pendiente en Action0.
    #[must_use]
    pub const fn has_slope_check_callback(&self) -> bool {
        (self.callback_mask & STATION_CALLBACK_SLOPE_CHECK_MASK) != 0
    }
}

/// Catálogo vanilla de clases.
#[must_use]
pub fn vanilla_station_class_catalog() -> Vec<StationClassDef> {
    vec![StationClassDef {
        id: StationClassId::DEFAULT,
        label: "Por defecto".into(),
        short_label: "Dflt".into(),
        from_newgrf: false,
    }]
}

/// Catálogo vanilla de specs.
#[must_use]
pub fn vanilla_station_spec_catalog() -> Vec<StationSpecDef> {
    vec![StationSpecDef {
        id: StationSpecId::DEFAULT_RAIL,
        class: StationClassId::DEFAULT,
        label: "Estación ferroviaria".into(),
        short_label: "Rail".into(),
        disallowed_platforms: 0,
        disallowed_lengths: 0,
        callback_mask: 0,
        flags: 0,
        animation_status: default_station_animation_status(),
        animation_frames: 0,
        animation_speed: default_station_animation_speed(),
        animation_triggers: 0,
        from_newgrf: false,
        newgrf_preview: None,
        newgrf_views: Vec::new(),
        newgrf_local_id: 0,
        newgrf_runtime: None,
        newgrf_grfid: 0,
        newgrf_grf_version: 0,
        newgrf_type_tables: None,
        custom_layouts: std::collections::HashMap::new(),
    }]
}

#[must_use]
pub fn all_station_class_defs() -> Vec<StationClassDef> {
    vanilla_station_class_catalog()
}

#[must_use]
pub fn all_station_spec_defs() -> Vec<StationSpecDef> {
    vanilla_station_spec_catalog()
}

#[must_use]
pub fn station_class_def(
    catalog: &[StationClassDef],
    id: StationClassId,
) -> Option<&StationClassDef> {
    catalog.iter().find(|c| c.id == id)
}

#[must_use]
pub fn station_spec_def(catalog: &[StationSpecDef], id: StationSpecId) -> Option<&StationSpecDef> {
    catalog.iter().find(|s| s.id == id)
}

#[must_use]
pub fn list_station_classes<'a>(
    catalog: &'a [StationClassDef],
    filter: &str,
) -> Vec<&'a StationClassDef> {
    let needle = filter.trim().to_ascii_lowercase();
    catalog
        .iter()
        .filter(|c| {
            if needle.is_empty() {
                return true;
            }
            c.label.to_ascii_lowercase().contains(&needle)
                || c.short_label.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

#[must_use]
pub fn list_station_specs<'a>(
    catalog: &'a [StationSpecDef],
    class: StationClassId,
    filter: &str,
) -> Vec<&'a StationSpecDef> {
    let needle = filter.trim().to_ascii_lowercase();
    catalog
        .iter()
        .filter(|s| s.class == class)
        .filter(|s| {
            if needle.is_empty() {
                return true;
            }
            s.label.to_ascii_lowercase().contains(&needle)
                || s.short_label.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

/// Índice de vista `NewGRF` Action1/2 desde `StationGfx` en `m5` (bits bajos).
///
/// Los layouts prop `0x0E` escriben el tiletype en `m5`; el render usa este
/// índice en lugar de hardcodear vista 0 (#46).
#[must_use]
pub fn station_newgrf_view_index(m5: u8) -> usize {
    usize::from(m5 & 0x0F)
}

/// Layout gfx; usa prop `0x0E` del spec si existe, si no vanilla.
#[must_use]
pub fn station_spec_layout(
    catalog: &[StationSpecDef],
    spec: StationSpecId,
    platforms: usize,
    length: usize,
) -> Vec<u8> {
    let p = u8::try_from(platforms).unwrap_or(0);
    let l = u8::try_from(length).unwrap_or(0);
    if let Some(def) = station_spec_def(catalog, spec)
        && let Some(layout) = def.custom_layouts.get(&(p, l))
        && layout.len() == platforms.saturating_mul(length)
    {
        return layout.clone();
    }
    crate::rail_station_layout(platforms, length)
}

/// `GetPlatformInfo` (`newgrf_station.cpp`): datos de plataforma para CB 24 / vars 40+.
#[must_use]
pub fn station_platform_info(
    gfx: u8,
    platforms: u8,
    length: u8,
    platform: u8,
    position: u8,
) -> u32 {
    let pos = u32::from(position.min(15));
    let dist_end = u32::from(length.saturating_sub(position).saturating_sub(1).min(15));
    let plat = u32::from(platform.min(15));
    let dist_side = u32::from(platforms.saturating_sub(platform).saturating_sub(1).min(15));
    let len = u32::from(length.min(15));
    let nplat = u32::from(platforms.min(15));
    pos | (dist_end << 4)
        | (plat << 8)
        | (dist_side << 12)
        | (len << 16)
        | (nplat << 20)
        | (u32::from(gfx) << 24)
}

/// Aplica callback 24 (`CBID_STATION_BUILD_TILE_LAYOUT`) sobre un tiletype de layout.
///
/// Si el callback falla o el spec no tiene runtime Action2, devuelve `base_gfx` (layout 0x0E).
#[must_use]
pub fn apply_station_build_tile_layout_callback(
    def: &StationSpecDef,
    base_gfx: u8,
    platforms: u8,
    length: u8,
    platform: u8,
    position: u8,
    axis_y: bool,
) -> u8 {
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return base_gfx;
    };
    let platinfo = station_platform_info(base_gfx, platforms, length, platform, position);
    let cb = runtime.resolve_callback(
        def.newgrf_local_id,
        crate::newgrf_sprites::CBID_STATION_BUILD_TILE_LAYOUT,
        platinfo,
        0,
    );
    if cb == crate::newgrf_sprites::CALLBACK_FAILED || cb > 0xFF {
        return base_gfx;
    }
    (u8::try_from(cb).unwrap_or(0) & !1) + u8::from(axis_y)
}

/// Aplica CB14 (`CBID_STATION_DRAW_TILE_LAYOUT`) al layout in-world de una tesela.
///
/// Un callback fallido, un resultado fuera de la representación `m5` local o
/// un spec sin máscara/runtime conserva `base_gfx`. El caller entrega el
/// contexto de la tesela para que el Action2 pueda leer sus vars de estación;
/// el scope/almacenamiento completo de `BaseStation` sigue fuera de este corte.
#[must_use]
pub fn apply_station_draw_tile_layout_callback(
    def: &StationSpecDef,
    base_gfx: u8,
    axis_y: bool,
    ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
) -> u8 {
    if !def.has_draw_tile_layout_callback() {
        return base_gfx;
    }
    let Some(runtime) = def.newgrf_runtime.as_ref() else {
        return base_gfx;
    };
    let cb = runtime.resolve_callback_ctx(
        def.newgrf_local_id,
        crate::newgrf_sprites::CBID_STATION_DRAW_TILE_LAYOUT,
        0,
        0,
        ctx,
    );
    if cb == crate::newgrf_sprites::CALLBACK_FAILED || cb > u16::from(u8::MAX) {
        return base_gfx;
    }
    (u8::try_from(cb).unwrap_or(base_gfx) & !1) | u8::from(axis_y)
}

#[must_use]
pub fn next_free_station_class_id(catalog: &[StationClassDef]) -> Option<StationClassId> {
    for id in 1u16..=1023 {
        let c = StationClassId::from_u16(id);
        if !catalog.iter().any(|d| d.id == c) {
            return Some(c);
        }
    }
    None
}

#[must_use]
pub fn next_free_station_spec_id(catalog: &[StationSpecDef]) -> Option<StationSpecId> {
    for id in 1u16..=1023 {
        let s = StationSpecId::from_u16(id);
        if !catalog.iter().any(|d| d.id == s) {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn list_filters_class_and_spec() {
        let classes = vanilla_station_class_catalog();
        let specs = vanilla_station_spec_catalog();
        assert_eq!(list_station_classes(&classes, "").len(), 1);
        assert_eq!(list_station_classes(&classes, "def").len(), 1);
        assert!(list_station_classes(&classes, "zzz").is_empty());

        let filtered = list_station_specs(&specs, StationClassId::Default, "ferro");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, StationSpecId::DefaultRail);
        assert!(list_station_specs(&specs, StationClassId::Default, "zzz").is_empty());
    }

    #[test]
    fn default_spec_allows_all_sizes() {
        let specs = vanilla_station_spec_catalog();
        let spec = station_spec_def(&specs, StationSpecId::DefaultRail).unwrap();
        for n in 1..=7u8 {
            assert!(spec.allows_platforms(n));
            assert!(spec.allows_length(n));
        }
    }

    #[test]
    fn disallowed_bitmask_blocks_size() {
        let mut spec = vanilla_station_spec_catalog().remove(0);
        spec.disallowed_platforms = 1 << 2;
        assert!(!spec.allows_platforms(3));
        assert!(spec.allows_platforms(2));
    }

    #[test]
    fn custom_layout_0e_overrides_vanilla() {
        let mut specs = vanilla_station_spec_catalog();
        specs[0].custom_layouts.insert((1, 3), vec![0, 2, 0]);
        let layout = station_spec_layout(&specs, StationSpecId::DefaultRail, 1, 3);
        assert_eq!(layout, vec![0, 2, 0]);
        let vanilla = station_spec_layout(&specs, StationSpecId::DefaultRail, 2, 2);
        assert_eq!(vanilla, crate::rail_station_layout(2, 2));
    }

    #[test]
    fn newgrf_view_index_uses_low_nibble_of_m5() {
        assert_eq!(station_newgrf_view_index(0x00), 0);
        assert_eq!(station_newgrf_view_index(0x02), 2);
        assert_eq!(station_newgrf_view_index(0x0F), 15);
        assert_eq!(station_newgrf_view_index(0x12), 2);
        assert_eq!(station_newgrf_view_index(0xA5), 5);
    }

    fn solid_sprite(r: u8, g: u8, b: u8) -> crate::newgrf_sprites::DecodedSprite {
        crate::newgrf_sprites::DecodedSprite {
            width: 2,
            height: 2,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![r, g, b, 255, r, g, b, 255, r, g, b, 255, r, g, b, 255],
            mask: Vec::new(),
        }
    }

    #[test]
    fn platform_info_matches_openttd_bit_layout() {
        let info = station_platform_info(3, 2, 3, 0, 1);
        // pos=1, dist_end=1, plat=0, dist_side=1, len=3, nplat=2, gfx=3
        assert_eq!(info & 0xF, 1);
        assert_eq!((info >> 4) & 0xF, 1);
        assert_eq!((info >> 8) & 0xF, 0);
        assert_eq!((info >> 12) & 0xF, 1);
        assert_eq!((info >> 16) & 0xF, 3);
        assert_eq!((info >> 20) & 0xF, 2);
        assert_eq!((info >> 24) & 0xFF, 3);
    }

    #[test]
    fn cb24_overrides_layout_tiletype_when_runtime_returns_literal() {
        use crate::newgrf_sprites::{
            Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
            TrainSpriteGraphics,
        };

        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 3,
        });
        // nvar=0: callback result = 4
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x1A,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: 4,
                        add_val: None,
                        divide_val: None,
                        modulo_val: None,
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        let mut specs = vanilla_station_spec_catalog();
        specs[0].newgrf_runtime = Some(Box::new(gfx));
        specs[0].newgrf_local_id = 0;
        // Layout 0x0E diría 0; CB24 fuerza 4 (+ axis).
        specs[0].custom_layouts.insert((1, 1), vec![0]);
        let out = apply_station_build_tile_layout_callback(&specs[0], 0, 1, 1, 0, 0, false);
        assert_eq!(out, 4);
        let out_y = apply_station_build_tile_layout_callback(&specs[0], 1, 1, 1, 0, 0, true);
        assert_eq!(out_y, 5); // 4|axis_y
    }

    #[test]
    fn cb14_selects_draw_layout_and_preserves_axis() {
        use crate::newgrf_sprites::{
            Action2EvalCtx, Action2VarAdjust, Action2VarEntry, Action2VarTerm, TrainSpriteAssign,
            TrainSpriteGraphics,
        };

        let mut gfx = TrainSpriteGraphics::default();
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 3,
        });
        // nvar=0: devuelve el callback id bajo (`0x14`) y prueba que CB14
        // llega al grafo, no sólo que se reutiliza el layout base.
        gfx.action2_var.insert(
            3,
            Action2VarEntry {
                first: Action2VarTerm {
                    variable: 0x0C,
                    param: None,
                    adjust: Action2VarAdjust {
                        shift: 0,
                        and_mask: u8::MAX,
                        ..Action2VarAdjust::default()
                    },
                },
                ops: Vec::new(),
                ranges: Vec::new(),
                default: 0,
            },
        );
        let mut specs = vanilla_station_spec_catalog();
        specs[0].callback_mask = STATION_CALLBACK_DRAW_TILE_LAYOUT_MASK;
        specs[0].newgrf_runtime = Some(Box::new(gfx));
        specs[0].newgrf_local_id = 0;

        let mut ctx = Action2EvalCtx::default();
        assert_eq!(
            apply_station_draw_tile_layout_callback(&specs[0], 2, true, &mut ctx),
            21,
            "0x14 con eje Y conserva el bit de eje tras limpiar el bit 0"
        );

        specs[0].callback_mask = 0;
        assert_eq!(
            apply_station_draw_tile_layout_callback(&specs[0], 2, true, &mut ctx),
            2,
            "sin bit DrawTileLayout conserva m5"
        );
        specs[0].callback_mask = STATION_CALLBACK_DRAW_TILE_LAYOUT_MASK;
        specs[0].newgrf_runtime = None;
        assert_eq!(
            apply_station_draw_tile_layout_callback(&specs[0], 2, true, &mut ctx),
            2,
            "sin runtime conserva el fallback"
        );
    }

    #[test]
    fn layout_0e_tiletypes_resolve_to_distinct_newgrf_views() {
        let mut specs = vanilla_station_spec_catalog();
        specs[0].custom_layouts.insert((1, 2), vec![0, 2]);
        specs[0].newgrf_views = vec![
            solid_sprite(255, 0, 0),
            solid_sprite(0, 255, 0),
            solid_sprite(0, 0, 255),
        ];
        let layout = station_spec_layout(&specs, StationSpecId::DefaultRail, 1, 2);
        assert_eq!(layout, vec![0, 2]);
        let def = station_spec_def(&specs, StationSpecId::DefaultRail).unwrap();
        let v0 = def
            .newgrf_view(station_newgrf_view_index(layout[0]))
            .unwrap();
        let v2 = def
            .newgrf_view(station_newgrf_view_index(layout[1]))
            .unwrap();
        assert_ne!(v0.rgba, v2.rgba);
        assert_eq!(v0.rgba[0], 255);
        assert_eq!(v2.rgba[2], 255);
    }
}
