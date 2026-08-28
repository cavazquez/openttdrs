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
/// | `m5`  | MAP5 (`MAP5`)  | Road bits (0-3), TrackBits (0-5), gfx industria (0-7), byte alto de `ObjectID` (MP_OBJECT) |
/// | `m1`  | MAP1 (chunk `MAPO`)  | Owner/índice de industria |
/// | `m6`  | MAP6 (`MAPE`)  | bit 2 = bit 8 del gfx de industria (9 bits totales); StationType en MP_STATION |
/// | `m8`  | MAP8 (`MAP8`)  | HouseID en MP_HOUSE (12 bits); RoadType tram en bits 6–11 en MP_ROAD (`road_map.h`) |
/// | `m3`  | M3LO (byte bajo de `m3`) | v4+: MP_HOUSE bit 7 = casa terminada; MP_ROAD bits 0–3 = tram track, 4–7 = owner tranvía |
/// | `m2`  | MAP2 | v5+: índice town/station/industry según tipo de tesela |
/// | `m7`  | MAP7 | v5+: reserva cruces, NewGRF en mapa, etc. |
/// | `m3hi` | M3HI | v5+: byte **`m4()`** del mapa OpenTTD (`M3HI` en `map_sl.cpp`; RoadType en bits 0–5, señales en nibble alto) |
/// | `m2_hi` | MAP2 hi | v5+12: byte alto de **`m2()`** 16-bit por tesela (reserva PBS en bits altos del save) |
///
/// Para `MP_RAILWAY`, TrackBits ocupa **bits 0-5** de m5 (6 bits); bits 6-7 son `RailTileType`.
/// Para `MP_INDUSTRY`, gfx = `m5 | ((m6 >> 2) & 1) << 8` (9 bits).
/// Para `MP_OBJECT`, m5 contiene el byte alto del `ObjectID`; el tipo se resuelve
/// desde el pool `OBJS`/footer `OBTY` mediante [`super::Map::object_type_at`].
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

/// Parámetros crudos de `MakeHouseTile` para una casa creada por un pueblo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TownHouseSpec {
    /// Índice de la especificación (`HouseID`) que ocupa los 12 bits bajos de `MAP8`.
    pub house_id: u16,
    /// Índice de pueblo que ocupa `MAP2`; se satura a `u16` al serializar.
    pub town_id: u32,
    /// Bits aleatorios iniciales de la casa (`MAP1`).
    pub random_bits: u8,
    /// Contador de la etapa de obra (`MAP5` bits 0–2).
    pub construction_counter: u8,
    /// Etapa de construcción (`MAP5` bits 3–4), o [`TOWN_HOUSE_COMPLETED`].
    pub construction_stage: u8,
    /// Bit de protección frente a reemplazos automáticos (`MAP3` bit 5).
    pub is_protected: bool,
    /// Multiplicador de refresco periódico (`MAP6` bits 2–7).
    pub processing_time: u8,
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

    /// Construye los bytes de `MakeHouseTile` para una casa de pueblo.
    ///
    /// Conserva el nibble bajo de `MAPT`, tal como `SetTileType(MP_HOUSE)`;
    /// éste puede contener zona tropical o metadatos de puente en la tesela
    /// que `CMD_LANDSCAPE_CLEAR` acaba de limpiar.
    #[must_use]
    pub fn town_house(spec: TownHouseSpec, height: u8, previous_mapt: u8) -> Self {
        let town_id = u16::try_from(spec.town_id).unwrap_or(u16::MAX);
        let [m2, m2_hi] = town_id.to_le_bytes();
        let completed = spec.construction_stage == TOWN_HOUSE_COMPLETED;
        let mut m3 = if completed { 0x80 } else { 0 };
        if spec.is_protected {
            m3 |= 0x20;
        }
        Self {
            height,
            kind: TileKind::House,
            mapt: (previous_mapt & 0x0F) | OTTD_MAPT_HOUSE,
            m5: if completed {
                0
            } else {
                ((spec.construction_stage & 0x03) << 3) | (spec.construction_counter & 0x07)
            },
            m1: spec.random_bits,
            m6: spec.processing_time.min(0x3F) << 2,
            m8: spec.house_id & 0x0FFF,
            m3,
            m2,
            m2_hi,
            m7: 0,
            m3hi: 0,
        }
    }
}

/// MAPT para `MP_HOUSE` (`TileType` 3).
pub const OTTD_MAPT_HOUSE: u8 = 0x30;
/// Etapa final de construcción de una casa (`TOWN_HOUSE_COMPLETED`).
pub const TOWN_HOUSE_COMPLETED: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    OutOfBounds,
}
