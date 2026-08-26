//! Helpers compartidos para caches NewGRF in-world (#135).

mod fingerprint;
mod image_factory;

pub(crate) use fingerprint::runtime_fingerprint;
pub(crate) use image_factory::{DecodedSpriteImagePolicy, decoded_sprite_image};

/// Listas de vars Action2 que entran en el fingerprint por dominio.
pub(crate) mod vars {
    pub const ROAD: &[u8] = &[0x40, 0x42, 0x45, 0x5F];
    pub const RAIL_SIGNAL: &[u8] = &[0x10, 0x18, 0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x5F];
    pub const INDUSTRY: &[u8] = &[0x40, 0x5F];
    /// Variables disponibles en `ObjectScopeResolver` para una tesela que ya
    /// está en el mapa: offset, terreno, animación, propietario y random.
    /// Las variables de town/teselas vecinas no se incluyen hasta que el
    /// renderer tenga el contexto global necesario.
    pub const OBJECT: &[u8] = &[0x40, 0x41, 0x43, 0x44, 0x5F];
    /// Variables de `HouseScopeResolver` presentes en `Tile`; zona, conteos y
    /// vecinos requieren el `Town`/estaciones globales y quedan documentados
    /// como residual.
    pub const HOUSE: &[u8] = &[0x40, 0x41, 0x43, 0x46, 0x47, 0x5F];
    pub const ROAD_STOP: &[u8] = &[0x40, 0x41, 0x42, 0x43, 0x44, 0x49, 0x50, 0x5F];
    pub const STATION: &[u8] = &[0x10, 0x40, 0x42, 0x43, 0x4A, 0x5F, 0x67];
    pub const TRAIN: &[u8] = &[0x40, 0x47, 0x43, 0x5F, 0xB2, 0xB4, 0xC8];
}
