//! Tablas `_roadveh_drive_data_*` / `_road_road_drive_data` (`table/roadveh_movement.h`).

/// Entrar en la siguiente tesela (`RDE_NEXT_TILE`).
pub const RDE_NEXT_TILE: u8 = 0x80;
/// Acaba de girar (`RDE_TURNED`).
pub const RDE_TURNED: u8 = 0x40;

/// Una entrada de trayectoria en tesela (`RoadDriveEntry`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoadDriveEntry {
    pub x: u8,
    pub y: u8,
}

impl RoadDriveEntry {
    #[must_use]
    pub const fn is_next_tile(self) -> bool {
        self.x & RDE_NEXT_TILE != 0
    }
    #[must_use]
    pub const fn is_turned(self) -> bool {
        self.x & RDE_TURNED != 0
    }
    #[must_use]
    pub const fn diagdir(self) -> u8 {
        self.x & 3
    }
}

static DATA_0: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 15, y: 5 },
    RoadDriveEntry { x: 14, y: 5 },
    RoadDriveEntry { x: 13, y: 5 },
    RoadDriveEntry { x: 12, y: 5 },
    RoadDriveEntry { x: 11, y: 5 },
    RoadDriveEntry { x: 10, y: 5 },
    RoadDriveEntry { x: 9, y: 5 },
    RoadDriveEntry { x: 8, y: 5 },
    RoadDriveEntry { x: 7, y: 5 },
    RoadDriveEntry { x: 6, y: 5 },
    RoadDriveEntry { x: 5, y: 5 },
    RoadDriveEntry { x: 4, y: 5 },
    RoadDriveEntry { x: 3, y: 5 },
    RoadDriveEntry { x: 2, y: 5 },
    RoadDriveEntry { x: 1, y: 5 },
    RoadDriveEntry { x: 0, y: 5 },
    RoadDriveEntry { x: 128, y: 0 },
];

static DATA_1: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 5, y: 0 },
    RoadDriveEntry { x: 5, y: 1 },
    RoadDriveEntry { x: 5, y: 2 },
    RoadDriveEntry { x: 5, y: 3 },
    RoadDriveEntry { x: 5, y: 4 },
    RoadDriveEntry { x: 5, y: 5 },
    RoadDriveEntry { x: 5, y: 6 },
    RoadDriveEntry { x: 5, y: 7 },
    RoadDriveEntry { x: 5, y: 8 },
    RoadDriveEntry { x: 5, y: 9 },
    RoadDriveEntry { x: 5, y: 10 },
    RoadDriveEntry { x: 5, y: 11 },
    RoadDriveEntry { x: 5, y: 12 },
    RoadDriveEntry { x: 5, y: 13 },
    RoadDriveEntry { x: 5, y: 14 },
    RoadDriveEntry { x: 5, y: 15 },
    RoadDriveEntry { x: 129, y: 0 },
];

static DATA_2: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 5, y: 0 },
    RoadDriveEntry { x: 5, y: 1 },
    RoadDriveEntry { x: 5, y: 2 },
    RoadDriveEntry { x: 4, y: 3 },
    RoadDriveEntry { x: 3, y: 4 },
    RoadDriveEntry { x: 2, y: 5 },
    RoadDriveEntry { x: 1, y: 5 },
    RoadDriveEntry { x: 0, y: 5 },
    RoadDriveEntry { x: 128, y: 0 },
];

static DATA_3: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 15, y: 5 },
    RoadDriveEntry { x: 14, y: 5 },
    RoadDriveEntry { x: 13, y: 5 },
    RoadDriveEntry { x: 12, y: 5 },
    RoadDriveEntry { x: 11, y: 5 },
    RoadDriveEntry { x: 10, y: 5 },
    RoadDriveEntry { x: 9, y: 6 },
    RoadDriveEntry { x: 8, y: 7 },
    RoadDriveEntry { x: 7, y: 8 },
    RoadDriveEntry { x: 6, y: 9 },
    RoadDriveEntry { x: 5, y: 10 },
    RoadDriveEntry { x: 5, y: 11 },
    RoadDriveEntry { x: 5, y: 12 },
    RoadDriveEntry { x: 5, y: 13 },
    RoadDriveEntry { x: 5, y: 14 },
    RoadDriveEntry { x: 5, y: 15 },
    RoadDriveEntry { x: 129, y: 0 },
];

static DATA_4: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 5, y: 0 },
    RoadDriveEntry { x: 5, y: 1 },
    RoadDriveEntry { x: 5, y: 2 },
    RoadDriveEntry { x: 5, y: 3 },
    RoadDriveEntry { x: 5, y: 4 },
    RoadDriveEntry { x: 5, y: 5 },
    RoadDriveEntry { x: 6, y: 6 },
    RoadDriveEntry { x: 7, y: 7 },
    RoadDriveEntry { x: 8, y: 8 },
    RoadDriveEntry { x: 9, y: 9 },
    RoadDriveEntry { x: 10, y: 9 },
    RoadDriveEntry { x: 11, y: 9 },
    RoadDriveEntry { x: 12, y: 9 },
    RoadDriveEntry { x: 13, y: 9 },
    RoadDriveEntry { x: 14, y: 9 },
    RoadDriveEntry { x: 15, y: 9 },
    RoadDriveEntry { x: 130, y: 0 },
];

static DATA_5: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 0, y: 9 },
    RoadDriveEntry { x: 1, y: 9 },
    RoadDriveEntry { x: 2, y: 9 },
    RoadDriveEntry { x: 3, y: 10 },
    RoadDriveEntry { x: 4, y: 11 },
    RoadDriveEntry { x: 5, y: 12 },
    RoadDriveEntry { x: 5, y: 13 },
    RoadDriveEntry { x: 5, y: 14 },
    RoadDriveEntry { x: 5, y: 15 },
    RoadDriveEntry { x: 129, y: 0 },
];

static DATA_6: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 0, y: 6 },
    RoadDriveEntry { x: 0, y: 7 },
    RoadDriveEntry { x: 0, y: 8 },
    RoadDriveEntry { x: 0, y: 9 },
    RoadDriveEntry { x: 66, y: 0 },
];

static DATA_7: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 6, y: 15 },
    RoadDriveEntry { x: 7, y: 15 },
    RoadDriveEntry { x: 8, y: 15 },
    RoadDriveEntry { x: 9, y: 15 },
    RoadDriveEntry { x: 67, y: 0 },
];

static DATA_8: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 0, y: 9 },
    RoadDriveEntry { x: 1, y: 9 },
    RoadDriveEntry { x: 2, y: 9 },
    RoadDriveEntry { x: 3, y: 9 },
    RoadDriveEntry { x: 4, y: 9 },
    RoadDriveEntry { x: 5, y: 9 },
    RoadDriveEntry { x: 6, y: 9 },
    RoadDriveEntry { x: 7, y: 9 },
    RoadDriveEntry { x: 8, y: 9 },
    RoadDriveEntry { x: 9, y: 9 },
    RoadDriveEntry { x: 10, y: 9 },
    RoadDriveEntry { x: 11, y: 9 },
    RoadDriveEntry { x: 12, y: 9 },
    RoadDriveEntry { x: 13, y: 9 },
    RoadDriveEntry { x: 14, y: 9 },
    RoadDriveEntry { x: 15, y: 9 },
    RoadDriveEntry { x: 130, y: 0 },
];

static DATA_9: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 9, y: 15 },
    RoadDriveEntry { x: 9, y: 14 },
    RoadDriveEntry { x: 9, y: 13 },
    RoadDriveEntry { x: 9, y: 12 },
    RoadDriveEntry { x: 9, y: 11 },
    RoadDriveEntry { x: 9, y: 10 },
    RoadDriveEntry { x: 9, y: 9 },
    RoadDriveEntry { x: 9, y: 8 },
    RoadDriveEntry { x: 9, y: 7 },
    RoadDriveEntry { x: 9, y: 6 },
    RoadDriveEntry { x: 9, y: 5 },
    RoadDriveEntry { x: 9, y: 4 },
    RoadDriveEntry { x: 9, y: 3 },
    RoadDriveEntry { x: 9, y: 2 },
    RoadDriveEntry { x: 9, y: 1 },
    RoadDriveEntry { x: 9, y: 0 },
    RoadDriveEntry { x: 131, y: 0 },
];

static DATA_10: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 0, y: 9 },
    RoadDriveEntry { x: 1, y: 9 },
    RoadDriveEntry { x: 2, y: 9 },
    RoadDriveEntry { x: 3, y: 9 },
    RoadDriveEntry { x: 4, y: 9 },
    RoadDriveEntry { x: 5, y: 9 },
    RoadDriveEntry { x: 6, y: 8 },
    RoadDriveEntry { x: 7, y: 7 },
    RoadDriveEntry { x: 8, y: 6 },
    RoadDriveEntry { x: 9, y: 5 },
    RoadDriveEntry { x: 9, y: 4 },
    RoadDriveEntry { x: 9, y: 3 },
    RoadDriveEntry { x: 9, y: 2 },
    RoadDriveEntry { x: 9, y: 1 },
    RoadDriveEntry { x: 9, y: 0 },
    RoadDriveEntry { x: 131, y: 0 },
];

static DATA_11: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 9, y: 15 },
    RoadDriveEntry { x: 9, y: 14 },
    RoadDriveEntry { x: 9, y: 13 },
    RoadDriveEntry { x: 10, y: 12 },
    RoadDriveEntry { x: 11, y: 11 },
    RoadDriveEntry { x: 12, y: 10 },
    RoadDriveEntry { x: 13, y: 9 },
    RoadDriveEntry { x: 14, y: 9 },
    RoadDriveEntry { x: 15, y: 9 },
    RoadDriveEntry { x: 130, y: 0 },
];

static DATA_12: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 15, y: 5 },
    RoadDriveEntry { x: 14, y: 5 },
    RoadDriveEntry { x: 13, y: 5 },
    RoadDriveEntry { x: 12, y: 4 },
    RoadDriveEntry { x: 11, y: 3 },
    RoadDriveEntry { x: 10, y: 2 },
    RoadDriveEntry { x: 9, y: 1 },
    RoadDriveEntry { x: 9, y: 0 },
    RoadDriveEntry { x: 131, y: 0 },
];

static DATA_13: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 9, y: 15 },
    RoadDriveEntry { x: 9, y: 14 },
    RoadDriveEntry { x: 9, y: 13 },
    RoadDriveEntry { x: 9, y: 12 },
    RoadDriveEntry { x: 9, y: 11 },
    RoadDriveEntry { x: 9, y: 10 },
    RoadDriveEntry { x: 8, y: 9 },
    RoadDriveEntry { x: 7, y: 8 },
    RoadDriveEntry { x: 6, y: 7 },
    RoadDriveEntry { x: 5, y: 6 },
    RoadDriveEntry { x: 4, y: 5 },
    RoadDriveEntry { x: 3, y: 5 },
    RoadDriveEntry { x: 2, y: 5 },
    RoadDriveEntry { x: 1, y: 5 },
    RoadDriveEntry { x: 0, y: 5 },
    RoadDriveEntry { x: 128, y: 0 },
];

static DATA_14: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 15, y: 8 },
    RoadDriveEntry { x: 15, y: 7 },
    RoadDriveEntry { x: 15, y: 6 },
    RoadDriveEntry { x: 15, y: 5 },
    RoadDriveEntry { x: 64, y: 0 },
];

static DATA_15: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 8, y: 0 },
    RoadDriveEntry { x: 7, y: 0 },
    RoadDriveEntry { x: 6, y: 0 },
    RoadDriveEntry { x: 5, y: 0 },
    RoadDriveEntry { x: 65, y: 0 },
];

static DATA_16: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 15, y: 9 },
    RoadDriveEntry { x: 14, y: 9 },
    RoadDriveEntry { x: 13, y: 9 },
    RoadDriveEntry { x: 12, y: 9 },
    RoadDriveEntry { x: 11, y: 9 },
    RoadDriveEntry { x: 10, y: 9 },
    RoadDriveEntry { x: 9, y: 9 },
    RoadDriveEntry { x: 8, y: 9 },
    RoadDriveEntry { x: 7, y: 9 },
    RoadDriveEntry { x: 6, y: 9 },
    RoadDriveEntry { x: 5, y: 9 },
    RoadDriveEntry { x: 4, y: 9 },
    RoadDriveEntry { x: 3, y: 9 },
    RoadDriveEntry { x: 2, y: 9 },
    RoadDriveEntry { x: 1, y: 9 },
    RoadDriveEntry { x: 0, y: 9 },
    RoadDriveEntry { x: 128, y: 0 },
];

static DATA_17: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 9, y: 0 },
    RoadDriveEntry { x: 9, y: 1 },
    RoadDriveEntry { x: 9, y: 2 },
    RoadDriveEntry { x: 9, y: 3 },
    RoadDriveEntry { x: 9, y: 4 },
    RoadDriveEntry { x: 9, y: 5 },
    RoadDriveEntry { x: 9, y: 6 },
    RoadDriveEntry { x: 9, y: 7 },
    RoadDriveEntry { x: 9, y: 8 },
    RoadDriveEntry { x: 9, y: 9 },
    RoadDriveEntry { x: 9, y: 10 },
    RoadDriveEntry { x: 9, y: 11 },
    RoadDriveEntry { x: 9, y: 12 },
    RoadDriveEntry { x: 9, y: 13 },
    RoadDriveEntry { x: 9, y: 14 },
    RoadDriveEntry { x: 9, y: 15 },
    RoadDriveEntry { x: 129, y: 0 },
];

static DATA_18: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 9, y: 0 },
    RoadDriveEntry { x: 9, y: 1 },
    RoadDriveEntry { x: 9, y: 2 },
    RoadDriveEntry { x: 9, y: 3 },
    RoadDriveEntry { x: 9, y: 4 },
    RoadDriveEntry { x: 9, y: 5 },
    RoadDriveEntry { x: 8, y: 6 },
    RoadDriveEntry { x: 7, y: 7 },
    RoadDriveEntry { x: 6, y: 8 },
    RoadDriveEntry { x: 5, y: 9 },
    RoadDriveEntry { x: 4, y: 9 },
    RoadDriveEntry { x: 3, y: 9 },
    RoadDriveEntry { x: 2, y: 9 },
    RoadDriveEntry { x: 1, y: 9 },
    RoadDriveEntry { x: 0, y: 9 },
    RoadDriveEntry { x: 128, y: 0 },
];

static DATA_19: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 15, y: 9 },
    RoadDriveEntry { x: 14, y: 9 },
    RoadDriveEntry { x: 13, y: 9 },
    RoadDriveEntry { x: 12, y: 10 },
    RoadDriveEntry { x: 11, y: 11 },
    RoadDriveEntry { x: 10, y: 12 },
    RoadDriveEntry { x: 9, y: 13 },
    RoadDriveEntry { x: 9, y: 14 },
    RoadDriveEntry { x: 9, y: 15 },
    RoadDriveEntry { x: 129, y: 0 },
];

static DATA_20: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 9, y: 0 },
    RoadDriveEntry { x: 9, y: 1 },
    RoadDriveEntry { x: 10, y: 2 },
    RoadDriveEntry { x: 11, y: 3 },
    RoadDriveEntry { x: 12, y: 4 },
    RoadDriveEntry { x: 13, y: 5 },
    RoadDriveEntry { x: 14, y: 5 },
    RoadDriveEntry { x: 15, y: 5 },
    RoadDriveEntry { x: 130, y: 0 },
];

static DATA_21: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 0, y: 5 },
    RoadDriveEntry { x: 1, y: 5 },
    RoadDriveEntry { x: 2, y: 5 },
    RoadDriveEntry { x: 3, y: 5 },
    RoadDriveEntry { x: 4, y: 5 },
    RoadDriveEntry { x: 5, y: 6 },
    RoadDriveEntry { x: 6, y: 7 },
    RoadDriveEntry { x: 7, y: 8 },
    RoadDriveEntry { x: 8, y: 9 },
    RoadDriveEntry { x: 9, y: 10 },
    RoadDriveEntry { x: 9, y: 11 },
    RoadDriveEntry { x: 9, y: 12 },
    RoadDriveEntry { x: 9, y: 13 },
    RoadDriveEntry { x: 9, y: 14 },
    RoadDriveEntry { x: 9, y: 15 },
    RoadDriveEntry { x: 129, y: 0 },
];

static DATA_22: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 0, y: 8 },
    RoadDriveEntry { x: 0, y: 7 },
    RoadDriveEntry { x: 0, y: 6 },
    RoadDriveEntry { x: 0, y: 5 },
    RoadDriveEntry { x: 66, y: 0 },
];

static DATA_23: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 8, y: 15 },
    RoadDriveEntry { x: 7, y: 15 },
    RoadDriveEntry { x: 6, y: 15 },
    RoadDriveEntry { x: 5, y: 15 },
    RoadDriveEntry { x: 67, y: 0 },
];

static DATA_24: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 0, y: 5 },
    RoadDriveEntry { x: 1, y: 5 },
    RoadDriveEntry { x: 2, y: 5 },
    RoadDriveEntry { x: 3, y: 5 },
    RoadDriveEntry { x: 4, y: 5 },
    RoadDriveEntry { x: 5, y: 5 },
    RoadDriveEntry { x: 6, y: 5 },
    RoadDriveEntry { x: 7, y: 5 },
    RoadDriveEntry { x: 8, y: 5 },
    RoadDriveEntry { x: 9, y: 5 },
    RoadDriveEntry { x: 10, y: 5 },
    RoadDriveEntry { x: 11, y: 5 },
    RoadDriveEntry { x: 12, y: 5 },
    RoadDriveEntry { x: 13, y: 5 },
    RoadDriveEntry { x: 14, y: 5 },
    RoadDriveEntry { x: 15, y: 5 },
    RoadDriveEntry { x: 130, y: 0 },
];

static DATA_25: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 5, y: 15 },
    RoadDriveEntry { x: 5, y: 14 },
    RoadDriveEntry { x: 5, y: 13 },
    RoadDriveEntry { x: 5, y: 12 },
    RoadDriveEntry { x: 5, y: 11 },
    RoadDriveEntry { x: 5, y: 10 },
    RoadDriveEntry { x: 5, y: 9 },
    RoadDriveEntry { x: 5, y: 8 },
    RoadDriveEntry { x: 5, y: 7 },
    RoadDriveEntry { x: 5, y: 6 },
    RoadDriveEntry { x: 5, y: 5 },
    RoadDriveEntry { x: 5, y: 4 },
    RoadDriveEntry { x: 5, y: 3 },
    RoadDriveEntry { x: 5, y: 2 },
    RoadDriveEntry { x: 5, y: 1 },
    RoadDriveEntry { x: 5, y: 0 },
    RoadDriveEntry { x: 131, y: 0 },
];

static DATA_26: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 0, y: 5 },
    RoadDriveEntry { x: 1, y: 5 },
    RoadDriveEntry { x: 2, y: 5 },
    RoadDriveEntry { x: 3, y: 4 },
    RoadDriveEntry { x: 4, y: 3 },
    RoadDriveEntry { x: 5, y: 2 },
    RoadDriveEntry { x: 5, y: 1 },
    RoadDriveEntry { x: 5, y: 0 },
    RoadDriveEntry { x: 131, y: 0 },
];

static DATA_27: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 5, y: 15 },
    RoadDriveEntry { x: 5, y: 14 },
    RoadDriveEntry { x: 5, y: 13 },
    RoadDriveEntry { x: 5, y: 12 },
    RoadDriveEntry { x: 5, y: 11 },
    RoadDriveEntry { x: 5, y: 10 },
    RoadDriveEntry { x: 6, y: 9 },
    RoadDriveEntry { x: 7, y: 8 },
    RoadDriveEntry { x: 8, y: 7 },
    RoadDriveEntry { x: 9, y: 6 },
    RoadDriveEntry { x: 10, y: 5 },
    RoadDriveEntry { x: 11, y: 5 },
    RoadDriveEntry { x: 12, y: 5 },
    RoadDriveEntry { x: 13, y: 5 },
    RoadDriveEntry { x: 14, y: 5 },
    RoadDriveEntry { x: 15, y: 5 },
    RoadDriveEntry { x: 130, y: 0 },
];

static DATA_28: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 15, y: 9 },
    RoadDriveEntry { x: 14, y: 9 },
    RoadDriveEntry { x: 13, y: 9 },
    RoadDriveEntry { x: 12, y: 9 },
    RoadDriveEntry { x: 11, y: 9 },
    RoadDriveEntry { x: 10, y: 9 },
    RoadDriveEntry { x: 9, y: 9 },
    RoadDriveEntry { x: 8, y: 8 },
    RoadDriveEntry { x: 7, y: 7 },
    RoadDriveEntry { x: 6, y: 6 },
    RoadDriveEntry { x: 5, y: 5 },
    RoadDriveEntry { x: 5, y: 4 },
    RoadDriveEntry { x: 5, y: 3 },
    RoadDriveEntry { x: 5, y: 2 },
    RoadDriveEntry { x: 5, y: 1 },
    RoadDriveEntry { x: 5, y: 0 },
    RoadDriveEntry { x: 131, y: 0 },
];

static DATA_29: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 5, y: 15 },
    RoadDriveEntry { x: 5, y: 14 },
    RoadDriveEntry { x: 5, y: 13 },
    RoadDriveEntry { x: 5, y: 12 },
    RoadDriveEntry { x: 4, y: 11 },
    RoadDriveEntry { x: 3, y: 10 },
    RoadDriveEntry { x: 2, y: 9 },
    RoadDriveEntry { x: 1, y: 9 },
    RoadDriveEntry { x: 0, y: 9 },
    RoadDriveEntry { x: 128, y: 0 },
];

static DATA_30: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 15, y: 6 },
    RoadDriveEntry { x: 15, y: 7 },
    RoadDriveEntry { x: 15, y: 8 },
    RoadDriveEntry { x: 15, y: 9 },
    RoadDriveEntry { x: 64, y: 0 },
];

static DATA_31: &[RoadDriveEntry] = &[
    RoadDriveEntry { x: 6, y: 0 },
    RoadDriveEntry { x: 7, y: 0 },
    RoadDriveEntry { x: 8, y: 0 },
    RoadDriveEntry { x: 9, y: 0 },
    RoadDriveEntry { x: 65, y: 0 },
];

/// Índice = trackdir (0..15) en carril estándar left-hand (`_road_road_drive_data`).
pub static ROAD_ROAD_DRIVE_DATA: &[&[RoadDriveEntry]] = &[
    DATA_0, DATA_1, DATA_2, DATA_3, DATA_4, DATA_5, DATA_6, DATA_7, DATA_8, DATA_9, DATA_10,
    DATA_11, DATA_12, DATA_13, DATA_14, DATA_15, DATA_16, DATA_17, DATA_18, DATA_19, DATA_20,
    DATA_21, DATA_22, DATA_23, DATA_24, DATA_25, DATA_26, DATA_27, DATA_28, DATA_29, DATA_30,
    DATA_31,
];

/// Entrada de conducción para `state` (trackdir 0..15 o con `RVSB_DRIVE_SIDE`) y `frame`.
#[must_use]
pub fn road_drive_entry(state: u8, frame: u8) -> Option<RoadDriveEntry> {
    let idx = usize::from(state & 0x1F);
    let table = ROAD_ROAD_DRIVE_DATA.get(idx)?;
    table.get(usize::from(frame)).copied()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn straight_ne_ends_with_next_tile() {
        let last = road_drive_entry(0, 16).unwrap();
        assert!(last.is_next_tile());
        assert_eq!(last.diagdir(), 0);
    }
}
