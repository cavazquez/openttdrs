//! Helpers compartidos para caches NewGRF in-world (#135).

mod fingerprint;
mod image_factory;

pub(crate) use fingerprint::runtime_fingerprint;
pub(crate) use image_factory::{
    DecodedSpriteImagePolicy, decoded_sprite_image, decoded_sprite_image_with_twocc_map,
};

/// Listas de vars Action2 que entran en el fingerprint por dominio.
pub(crate) mod vars {
    pub const ROAD: &[u8] = &[0x40, 0x42, 0x45, 0x5F];
    pub const RAIL_SIGNAL: &[u8] = &[0x10, 0x18, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x5F];
    /// Variables de `IndustryTileScopeResolver` y entradas básicas del
    /// `IndustriesScopeResolver` que pueden cambiar la vista runtime.
    pub const INDUSTRY: &[u8] = &[0x40, 0x41, 0x42, 0x43, 0x44, 0x5F, 0x7A];
    /// Variables disponibles en `ObjectScopeResolver` para una tesela que ya
    /// está en el mapa: offset, terreno, pueblo/distancias, animación,
    /// propietario y random. Las variables de teselas vecinas siguen fuera
    /// del fingerprint hasta completar su contexto global.
    pub const OBJECT: &[u8] = &[0x40, 0x41, 0x43, 0x44, 0x45, 0x46, 0x5F];
    /// Variables de `HouseScopeResolver` presentes en `Tile`, pueblo,
    /// conteos precalculados y vecinos de la tesela.
    pub const HOUSE: &[u8] = &[
        0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x60, 0x61, 0x62, 0x63, 0x5F,
    ];
    pub const ROAD_STOP: &[u8] = &[0x40, 0x41, 0x42, 0x43, 0x44, 0x49, 0x50, 0x5F];
    pub const STATION: &[u8] = &[0x10, 0x40, 0x42, 0x43, 0x4A, 0x5F, 0x67];
    /// Variables `AirportTileScopeResolver` que pueden cambiar la vista por
    /// posición, frame o estado de una tesela vecina. Las tablas
    /// `parameterized_vars` se incorporan además por `runtime_fingerprint`.
    pub const AIRPORT_TILE: &[u8] = &[0x41, 0x42, 0x43, 0x44, 0x5F, 0x60, 0x61, 0x62, 0x7A];
    pub const TRAIN: &[u8] = &[0x10, 0x40, 0x47, 0x43, 0x5F, 0xB2, 0xB4, 0xC8];
}
