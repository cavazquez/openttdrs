/// Coordenada de tesela en el plano X/Y del mapa (análoga a índices de tesela en `OpenTTD`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TileCoord {
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Nibble alto de `mapt` / MAPT: `TileType::TunnelBridge` en OpenTTD (= 9).
pub const OTTD_TILETYPE_TUNNELBRIDGE: u8 = 9;

/// Convierte un `TileIndex` de OpenTTD a coordenadas cuando el mapa es potencia de 2 en X e Y
/// (misma convención que `TileXY`: `tile = x | (y << log2(map_w))`).
#[must_use]
pub fn openttd_tile_index_to_coord(tile: u32, map_w: u32, map_h: u32) -> Option<TileCoord> {
    if !map_w.is_power_of_two() || !map_h.is_power_of_two() {
        return None;
    }
    let log_w = map_w.trailing_zeros();
    let x = tile & (map_w - 1);
    let y = tile >> log_w;
    if y >= map_h {
        return None;
    }
    let xi = i32::try_from(x).ok()?;
    let yi = i32::try_from(y).ok()?;
    Some(TileCoord::new(xi, yi))
}

/// Tipo semántico de una tesela.
///
/// Cubre los tipos de `TileType` de `OpenTTD` necesarios para el renderer.
/// Los tipos sin sprite dedicado se renderizan con un color de fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TileKind {
    #[default]
    Grass,
    Water,
    Forest,
    CoalField,
    Road,
    Rail,
    RoadDepot,
    RailDepot,
    /// Depósito de barcos (MVP simplificado).
    ShipDepot,
    /// Aeropuerto / hangar (MVP simplificado).
    Airport,
    RoadTunnel,
    RailTunnel,
    RoadBridge,
    RailBridge,
    House,       // MP_HOUSE  (3) – edificio urbano
    Station,     // MP_STATION (5)
    Industry,    // MP_INDUSTRY (8) – genérico (sin sub-tipo conocido)
    Void,        // MP_VOID (7) – borde del mapa
    Unknown(u8), // cualquier tipo no mapeado (raw nibble alto de tile_type)
}

/// Una tesela con altura base, tipo semántico y bytes auxiliares de `OpenTTD`.
///
/// Datos de mapa OpenTTD para una tesela.
///
/// | Campo | Fuente     | Uso principal |
/// |-------|-----------|---------------|
/// | `m5`  | MAP5 (`MAP5`)  | Road bits (0-3), TrackBits (0-5), gfx industria (0-7), ObjectType (MP_OBJECT) |
/// | `m1`  | MAP1 (chunk `MAPO`)  | Owner/índice de industria |
/// | `m6`  | MAP6 (`MAPE`)  | bit 2 = bit 8 del gfx de industria (9 bits totales); StationType en MP_STATION |
/// | `m8`  | MAP8 (`MAP8`)  | HouseID en MP_HOUSE (12 bits); RoadType tram en bits 6–11 en MP_ROAD (`road_map.h`) |
/// | `m3`  | M3LO (byte bajo de `m3`) | v4+: MP_HOUSE bit 7 = casa terminada; MP_ROAD bits 0–3 = tram track, 4–7 = owner tranvía |
/// | `m2`  | MAP2 | v5+: índice town/station/industry según tipo de tesela |
/// | `m7`  | MAP7 | v5+: reserva cruces, NewGRF en mapa, etc. |
/// | `m3hi` | M3HI | v5+: byte **`m4()`** del mapa OpenTTD (`M3HI` en `map_sl.cpp`; señales: `GetSignalStates` en nibble alto) |
/// | `m2_hi` | MAP2 hi | v5+12: byte alto de **`m2()`** 16-bit por tesela (reserva PBS en bits altos del save) |
///
/// Para `MP_RAILWAY`, TrackBits ocupa **bits 0-5** de m5 (6 bits); bits 6-7 son `RailTileType`.
/// Para `MP_INDUSTRY`, gfx = `m5 | ((m6 >> 2) & 1) << 8` (9 bits).
/// Para `MP_OBJECT`, m5 contiene el `ObjectType` (precomputado por `parse_sav.py` desde OBJS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tile {
    pub height: u8,
    pub kind: TileKind,
    /// Byte MAPT del savegame (nibble alto = `TileType` `OpenTTD`). 0 en mapas generados.
    pub mapt: u8,
    pub m5: u8,
    /// Byte M1 del savegame. Para industrias: bits 0-6 = índice de industria.
    pub m1: u8,
    /// Byte M6 del savegame. bit 2 = bit 8 del gfx de industria.
    pub m6: u8,
    /// Bytes M8 del savegame (little-endian, 2 bytes). HouseID en MP_HOUSE.
    pub m8: u16,
    /// Byte M3LO del savegame (`.ottdmap` v4+). `0` si el archivo no incluye la sección.
    pub m3: u8,
    /// Byte bajo de MAP2 (`.ottdmap` v5+); en el save OpenTTD `m2()` es `u16` LE.
    pub m2: u8,
    /// Byte alto de MAP2 (`.ottdmap` v5+12); `0` si el archivo no incluye el plano extra.
    pub m2_hi: u8,
    /// Byte MAP7 (`.ottdmap` v5+).
    pub m7: u8,
    /// Byte M3HI = **`m4()`** en OpenTTD (`.ottdmap` v5+).
    pub m3hi: u8,
}

impl Tile {
    /// Nibble alto del tipo de tesela OpenTTD (`mapt >> 4`).
    #[must_use]
    pub fn ottd_type_nibble(self) -> u8 {
        (self.mapt >> 4) & 0x0F
    }

    /// `true` si MAPT indica `MP_TUNNELBRIDGE`.
    #[must_use]
    pub fn is_tunnel_bridge_tile(self) -> bool {
        self.ottd_type_nibble() == OTTD_TILETYPE_TUNNELBRIDGE
    }

    /// Casa terminada (`IsHouseCompleted` en bit 7 de `m3`; HouseID en `m8`).
    #[must_use]
    pub fn completed_house(house_id: u16, age: u8, height: u8) -> Self {
        Self {
            height,
            kind: TileKind::House,
            mapt: OTTD_MAPT_HOUSE,
            m5: age,
            m1: 0,
            m6: 0,
            m8: house_id & 0xFFF,
            m3: 0x80,
            m2: 0,
            m2_hi: 0,
            m7: 0,
            m3hi: 0,
        }
    }
}

/// MAPT para `MP_HOUSE` (`TileType` 3).
pub const OTTD_MAPT_HOUSE: u8 = 0x30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    OutOfBounds,
}
