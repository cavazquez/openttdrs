// Generado por scripts/gen_house_population.py — NO EDITAR A MANO.
//
// `HouseSpec::population` por HouseID original (3er argumento de las
// macros `MS` en `_original_house_specs`, `table/town_land.h`).
// Usado para reconstruir `Town::cache.population` como
// `RebuildTownCaches` (`town_sl.cpp`).
//
// `HOUSE_MAIL_GENERATION`: `HouseSpec::mail_generation` (7.º arg de `MS`).
// `HOUSE_SIZE_1X1`: `BuildingFlag::Size1x1` (footprint de una tesela).

pub(crate) static HOUSE_POPULATION: [u16; 110] = [
    187, 85, 40, 5, 220, 220, 30, 140, 0, 0, // 0..9
    0, 0, 0, 150, 95, 95, 95, 130, 6, 110, // 10..19
    65, 0, 0, 0, 15, 12, 13, 100, 170, 100, // 20..29
    180, 35, 65, 0, 0, 0, 140, 15, 15, 35, // 30..39
    180, 0, 0, 0, 80, 80, 16, 16, 14, 14, // 40..49
    135, 135, 170, 170, 210, 210, 10, 10, 25, 25, // 50..59
    6, 6, 17, 17, 90, 90, 140, 0, 140, 0, // 60..69
    105, 105, 190, 190, 250, 0, 250, 0, 16, 16, // 70..79
    16, 7, 45, 8, 18, 90, 120, 250, 0, 80, // 80..89
    180, 8, 18, 7, 15, 17, 19, 21, 75, 35, // 90..99
    0, 85, 11, 10, 67, 86, 95, 30, 25, 18, // 100..109
];

pub(crate) static HOUSE_MAIL_GENERATION: [u16; 110] = [
    70, 55, 20, 2, 85, 85, 12, 22, 22, 0, // 0..9
    0, 0, 0, 65, 48, 48, 48, 50, 10, 55, // 10..19
    5, 5, 5, 5, 6, 7, 8, 35, 50, 40, // 20..29
    64, 23, 5, 5, 5, 5, 65, 6, 6, 23, // 30..39
    5, 5, 5, 5, 20, 20, 6, 6, 6, 6, // 40..49
    60, 60, 70, 70, 80, 80, 5, 5, 20, 20, // 50..59
    2, 2, 7, 7, 45, 45, 25, 25, 25, 25, // 60..69
    50, 50, 75, 75, 60, 60, 60, 60, 6, 6, // 70..79
    5, 4, 15, 3, 7, 24, 25, 80, 80, 23, // 80..89
    90, 3, 5, 3, 6, 6, 6, 6, 20, 9, // 90..99
    0, 18, 3, 3, 22, 23, 28, 10, 8, 7, // 100..109
];

pub(crate) static HOUSE_SIZE_1X1: [bool; 110] = [
    true, true, true, true, true, true, true, false, false, true, // 0..9
    true, true, true, true, true, true, true, true, true, true, // 10..19
    false, false, false, false, true, true, true, true, true, true, // 20..29
    true, true, false, false, false, false, true, true, true, true, // 30..39
    false, false, false, false, true, true, true, true, true, true, // 40..49
    true, true, true, true, true, true, true, true, true, true, // 50..59
    true, true, true, true, true, true, false, false, false, false, // 60..69
    true, true, true, true, false, false, false, false, true, true, // 70..79
    true, true, true, true, true, true, true, false, false, true, // 80..89
    true, true, true, true, true, true, true, true, true, false, // 90..99
    false, true, true, true, true, true, true, true, true, true, // 100..109
];
