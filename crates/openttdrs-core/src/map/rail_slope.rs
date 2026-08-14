//! Validación vía + pendiente según `GetRailFoundation` / `CheckRailSlope` de OpenTTD
//! (`rail_cmd.cpp`, tablas `_valid_tracks_without_foundation` y
//! `_valid_tracks_on_leveled_foundation`).

use super::slope::{SLOPE_STEEP, complement_slope};

/// Esquinas individuales (`slope_type.h`).
const SLOPE_W: u8 = 0x01;
const SLOPE_S: u8 = 0x02;
const SLOPE_E: u8 = 0x04;
const SLOPE_N: u8 = 0x08;

/// `TrackBits` (`track_type.h`).
const TRACK_BIT_X: u8 = 1;
const TRACK_BIT_Y: u8 = 2;
const TRACK_BIT_UPPER: u8 = 4;
const TRACK_BIT_LOWER: u8 = 8;
const TRACK_BIT_LEFT: u8 = 16;
const TRACK_BIT_RIGHT: u8 = 32;
const TRACK_BIT_HORZ: u8 = TRACK_BIT_UPPER | TRACK_BIT_LOWER;
const TRACK_BIT_VERT: u8 = TRACK_BIT_LEFT | TRACK_BIT_RIGHT;

const FOUNDATION_INVALID: u8 = 0xFF;
/// `Foundation::Leveled` de OpenTTD.
///
/// Además de las vías, las cabezas de puente usan este tipo cuando deben
/// aplanar una pendiente sin una esquina alta única.
pub const FOUNDATION_LEVELED: u8 = 1;
/// `Foundation::InclinedX` de OpenTTD.
pub const FOUNDATION_INCLINED_X: u8 = 2;
/// `Foundation::InclinedY` de OpenTTD.
pub const FOUNDATION_INCLINED_Y: u8 = 3;
const FOUNDATION_STEEP_LOWER: u8 = 4;
const FOUNDATION_STEEP_BOTH: u8 = 5;
const FOUNDATION_HALFTILE_W: u8 = 6;
const FOUNDATION_HALFTILE_N: u8 = 9;
const FOUNDATION_RAIL_W: u8 = 10;
const FOUNDATION_RAIL_N: u8 = 13;
const SLOPE_HALFTILE: u8 = 0x20;

/// Primer sprite clásico de cimientos nivelados (`SPR_FOUNDATION_BASE`).
pub const FOUNDATION_ORIGINAL_SPRITE_BASE: u32 = 989;
/// Primer slot Action5 de cimientos extra (`SPR_SLOPES_BASE`).
pub const FOUNDATION_ACTION5_SPRITE_BASE: u32 = 5413;

const FOUNDATION_SLOPES_VIRTUAL_BASE: u32 = FOUNDATION_ACTION5_SPRITE_BASE - 15;
const FOUNDATION_INCLINED_OFFSET: u32 = 15;
const FOUNDATION_BLOCK_SIZE: u32 = 22;
const FOUNDATION_HALFTILE_OFFSET: u32 = 74;
const FOUNDATION_HALFTILE_BLOCK_SIZE: u32 = 4;

/// Un sprite que `DrawFoundation` manda a dibujar para una vía.
///
/// `z_delta` está expresado en unidades de altura de tile (8 píxeles en
/// OpenTTD) y es relativo a la base de la tesela original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RailFoundationSpriteDraw {
    pub sprite_id: u32,
    pub z_delta: u8,
}

/// Selección visual completa de `DrawTrackBits` + `DrawFoundation`.
///
/// La mayor combinación posible es la parte inferior y superior de una
/// pendiente empinada, por eso no hace falta asignar un `Vec` por tesela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RailFoundationDrawPlan {
    pub sprites: [Option<RailFoundationSpriteDraw>; 2],
    pub surface_tileh: u8,
    pub surface_z_delta: u8,
}

/// Una pasada de `DrawTrackBits` para una vía sobre su cimiento.
///
/// Las fundaciones de medio bloque no son una única superficie continua: el
/// código de OpenTTD pinta primero las vías que quedaron en la parte baja y,
/// tras llamar a `DrawFoundation(Halftile...)`, vuelve a pintar la vía de la
/// mitad alta usando una pendiente falsa de tres esquinas elevadas.  Conservar
/// ambas pasadas evita seleccionar el sprite de la pendiente original para la
/// parte alta (por ejemplo, `1023` donde OpenTTD usa `1030`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RailTrackSpritePass {
    /// `TrackBits` presentes en esta pasada.
    pub track_bits: u8,
    /// Pendiente continua que selecciona `_track_sloped_sprites`.
    ///
    /// Nunca contiene la codificación interna `SLOPE_HALFTILE`: en la pasada
    /// alta OpenTTD selecciona explícitamente una pendiente falsa continua.
    pub sprite_tileh: u8,
    /// Elevación (en unidades de tile) tras las fundaciones aplicadas antes de
    /// dibujar esta pasada.
    pub z_delta: u8,
    /// Esquina que recorta esta pasada a la mitad alta, si corresponde.
    /// El renderer que soporte `SubSprite` debe aplicar el mismo recorte que
    /// `_halftile_sub_sprite` de `rail_cmd.cpp`.
    pub halftile_corner: Option<u8>,
}

/// Plan de las una o dos pasadas que hace `DrawTrackBits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RailTrackDrawPlan {
    pub passes: [Option<RailTrackSpritePass>; 2],
}

/// Esquinas de tesela (`Corner` en `slope_type.h`).
const CORNER_W: u8 = 0;
const CORNER_S: u8 = 1;
const CORNER_E: u8 = 2;
const CORNER_N: u8 = 3;

const SLOPE_NWS: u8 = SLOPE_N | SLOPE_W | SLOPE_S;
const SLOPE_WSE: u8 = SLOPE_W | SLOPE_S | SLOPE_E;
const SLOPE_SEN: u8 = SLOPE_S | SLOPE_E | SLOPE_N;
const SLOPE_STEEP_W: u8 = SLOPE_STEEP | SLOPE_NWS;
const SLOPE_STEEP_S: u8 = SLOPE_STEEP | SLOPE_WSE;
const SLOPE_STEEP_E: u8 = SLOPE_STEEP | SLOPE_SEN;
const SLOPE_STEEP_N: u8 = SLOPE_STEEP | SLOPE_N | SLOPE_E | SLOPE_W;
const VALID_TRACKS_WITHOUT_FOUNDATION: [u8; 15] = [
    0x3F, 0x20, 0x04, 0x01, 0x10, 0x00, 0x02, 0x08, 0x08, 0x02, 0x00, 0x10, 0x01, 0x04, 0x20,
];

/// `_valid_tracks_on_leveled_foundation` en `rail_cmd.cpp`.
const VALID_TRACKS_ON_LEVELED_FOUNDATION: [u8; 15] = [
    0x00, 0x10, 0x08, 0x1A, 0x20, 0x3F, 0x29, 0x3F, 0x04, 0x15, 0x3F, 0x3F, 0x26, 0x3F, 0x3F,
];

#[inline]
const fn is_steep_slope(tileh: u8) -> bool {
    tileh & SLOPE_STEEP != 0
}

#[inline]
const fn is_slope_with_one_corner_raised(tileh: u8) -> bool {
    matches!(tileh, SLOPE_W | SLOPE_S | SLOPE_E | SLOPE_N)
}

#[inline]
const fn slope_with_one_corner_raised(corner: u8) -> u8 {
    1 << corner
}

#[inline]
const fn opposite_corner(corner: u8) -> u8 {
    corner ^ 2
}

#[inline]
const fn slope_with_three_corners_raised(corner: u8) -> u8 {
    complement_slope(slope_with_one_corner_raised(corner))
}

#[inline]
const fn is_slope_with_three_corners_raised(tileh: u8) -> bool {
    !is_steep_slope(tileh) && is_slope_with_one_corner_raised(complement_slope(tileh))
}

#[inline]
const fn corner_to_track_bits(corner: u8) -> u8 {
    match corner {
        CORNER_W => TRACK_BIT_LEFT,
        CORNER_S => TRACK_BIT_LOWER,
        CORNER_E => TRACK_BIT_RIGHT,
        _ => TRACK_BIT_UPPER,
    }
}

/// `TracksOverlap` en `track_func.h`.
#[inline]
const fn tracks_overlap(bits: u8) -> bool {
    if bits == 0 {
        return false;
    }
    let without_first = bits & (bits - 1);
    if without_first == 0 {
        return false;
    }
    bits != TRACK_BIT_HORZ && bits != TRACK_BIT_VERT
}

#[inline]
const fn highest_slope_corner(tileh: u8) -> u8 {
    match tileh & !0xE0 {
        SLOPE_W | SLOPE_STEEP_W => CORNER_W,
        SLOPE_S | SLOPE_STEEP_S => CORNER_S,
        SLOPE_E | SLOPE_STEEP_E => CORNER_E,
        _ => CORNER_N,
    }
}

#[inline]
const fn halftile_foundation(corner: u8) -> u8 {
    6 + corner
}

#[inline]
const fn special_rail_foundation(corner: u8) -> u8 {
    10 + corner
}

/// Réplica de `GetRailFoundation` (`rail_cmd.cpp`). Devuelve `FOUNDATION_INVALID` (0xFF)
/// si la combinación pendiente + `TrackBits` no es construible.
#[must_use]
pub fn rail_foundation_for_trackbits(tileh: u8, bits: u8) -> u8 {
    let bits = bits & 0x3F;
    if bits == 0 {
        return 0;
    }

    if is_steep_slope(tileh) {
        if bits == TRACK_BIT_X {
            return 2;
        }
        if bits == TRACK_BIT_Y {
            return 3;
        }
        let highest = highest_slope_corner(tileh);
        let higher_track = corner_to_track_bits(highest);
        if bits == higher_track {
            return halftile_foundation(highest);
        }
        if tracks_overlap(bits | higher_track) {
            return FOUNDATION_INVALID;
        }
        return if bits & higher_track != 0 { 5 } else { 4 };
    }

    let tileh_idx = usize::from(tileh.min(14));
    if bits & !VALID_TRACKS_WITHOUT_FOUNDATION[tileh_idx] == 0 {
        return 0;
    }

    let valid_on_leveled = bits & !VALID_TRACKS_ON_LEVELED_FOUNDATION[tileh_idx] == 0;

    let track_corner = match bits {
        TRACK_BIT_LEFT => CORNER_W,
        TRACK_BIT_LOWER => CORNER_S,
        TRACK_BIT_RIGHT => CORNER_E,
        TRACK_BIT_UPPER => CORNER_N,
        TRACK_BIT_HORZ => {
            if tileh == SLOPE_N {
                return halftile_foundation(CORNER_N);
            }
            if tileh == SLOPE_S {
                return halftile_foundation(CORNER_S);
            }
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
        TRACK_BIT_VERT => {
            if tileh == SLOPE_W {
                return halftile_foundation(CORNER_W);
            }
            if tileh == SLOPE_E {
                return halftile_foundation(CORNER_E);
            }
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
        TRACK_BIT_X => {
            if is_slope_with_one_corner_raised(tileh) {
                return 2;
            }
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
        TRACK_BIT_Y => {
            if is_slope_with_one_corner_raised(tileh) {
                return 3;
            }
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
        _ => {
            return if valid_on_leveled {
                1
            } else {
                FOUNDATION_INVALID
            };
        }
    };

    if !valid_on_leveled {
        return FOUNDATION_INVALID;
    }
    if is_slope_with_three_corners_raised(tileh) {
        return 1;
    }
    if (tileh & slope_with_three_corners_raised(opposite_corner(track_corner)))
        == slope_with_one_corner_raised(track_corner)
    {
        return halftile_foundation(track_corner);
    }
    special_rail_foundation(track_corner)
}

/// Pendiente y elevación de la superficie de una vía después de aplicar su
/// cimiento. Replica `ApplyFoundationToSlope(GetRailFoundation(...))`.
///
/// El primer componente puede contener la codificación de medio bloque de
/// OpenTTD (`SLOPE_HALFTILE`); los consumidores que sólo manejan pendientes
/// continuas deben tratar ese caso por separado.
#[must_use]
pub fn rail_surface_slope_and_z(tileh: u8, bits: u8) -> (u8, u8) {
    let foundation = rail_foundation_for_trackbits(tileh, bits);
    if foundation == 0 || foundation == FOUNDATION_INVALID {
        return (tileh, 0);
    }

    let steep_z = u8::from(is_steep_slope(tileh));
    match foundation {
        FOUNDATION_LEVELED => (0, 1 + steep_z),
        FOUNDATION_HALFTILE_W..=FOUNDATION_HALFTILE_N => {
            let corner = foundation - FOUNDATION_HALFTILE_W;
            (tileh | SLOPE_HALFTILE | (corner << 6), 0)
        }
        FOUNDATION_RAIL_W..=FOUNDATION_RAIL_N => {
            let corner = foundation - FOUNDATION_RAIL_W;
            (slope_with_three_corners_raised(opposite_corner(corner)), 0)
        }
        FOUNDATION_INCLINED_X => {
            let highest = highest_slope_corner(tileh);
            let slope = if highest == CORNER_W || highest == CORNER_S {
                SLOPE_S | SLOPE_W
            } else {
                SLOPE_N | SLOPE_E
            };
            (slope, steep_z)
        }
        FOUNDATION_INCLINED_Y => {
            let highest = highest_slope_corner(tileh);
            let slope = if highest == CORNER_S || highest == CORNER_E {
                SLOPE_S | SLOPE_E
            } else {
                SLOPE_N | SLOPE_W
            };
            (slope, steep_z)
        }
        FOUNDATION_STEEP_LOWER => (
            slope_with_one_corner_raised(highest_slope_corner(tileh)),
            steep_z,
        ),
        FOUNDATION_STEEP_BOTH => {
            let highest = highest_slope_corner(tileh);
            (
                slope_with_one_corner_raised(highest) | SLOPE_HALFTILE | (highest << 6),
                steep_z,
            )
        }
        _ => (tileh, 0),
    }
}

/// `GetBridgeFoundation` para una cabeza de puente, expresado con el eje de
/// la vía (`true` = X, `false` = Y).
///
/// Las rampas usan las mismas variantes `Leveled` / `InclinedX` /
/// `InclinedY` que las fundaciones ferroviarias, pero la elección no depende
/// de los seis `TrackBits`: depende exclusivamente del eje del puente.
#[must_use]
pub fn bridge_foundation_for_axis(tileh: u8, axis_x: bool) -> u8 {
    let aligned_with_axis = if axis_x {
        tileh == (SLOPE_N | SLOPE_E) || tileh == (SLOPE_W | SLOPE_S)
    } else {
        tileh == (SLOPE_N | SLOPE_W) || tileh == (SLOPE_S | SLOPE_E)
    };
    if tileh == 0 || aligned_with_axis {
        return 0;
    }

    if bridge_highest_corner(tileh).is_some() {
        if axis_x {
            FOUNDATION_INCLINED_X
        } else {
            FOUNDATION_INCLINED_Y
        }
    } else {
        FOUNDATION_LEVELED
    }
}

/// `ApplyFoundationToSlope(GetBridgeFoundation(...))`.
///
/// Devuelve la pendiente y el incremento de Z que recibiría el `TileInfo`
/// antes de que se dibujen rampa, pilares o catenaria.
///
/// # Panics
///
/// Panics si se solicita una fundación inclinada para una pendiente sin una
/// única esquina alta. `bridge_foundation_for_axis` no produce esa combinación.
#[must_use]
pub fn bridge_surface_slope_and_z(tileh: u8, axis_x: bool) -> (u8, u8) {
    let foundation = bridge_foundation_for_axis(tileh, axis_x);
    if foundation == 0 {
        return (tileh, 0);
    }

    let steep_z = u8::from(is_steep_slope(tileh));
    if foundation == FOUNDATION_LEVELED {
        return (0, 1 + steep_z);
    }

    let highest = bridge_highest_corner(tileh)
        .expect("una fundación inclinada de puente requiere una esquina alta");
    let surface = if axis_x {
        if matches!(highest, CORNER_W | CORNER_S) {
            SLOPE_S | SLOPE_W
        } else {
            SLOPE_N | SLOPE_E
        }
    } else if matches!(highest, CORNER_S | CORNER_E) {
        SLOPE_S | SLOPE_E
    } else {
        SLOPE_N | SLOPE_W
    };
    (surface, steep_z)
}

/// `HasSlopeHighestCorner` / `GetHighestSlopeCorner` para el subconjunto que
/// tiene una única esquina alta. Una pendiente de tres esquinas elevadas no
/// entra: tiene una esquina baja y por ello recibe fundación nivelada.
#[inline]
const fn bridge_highest_corner(tileh: u8) -> Option<u8> {
    match tileh & !0xE0 {
        SLOPE_W | SLOPE_STEEP_W => Some(CORNER_W),
        SLOPE_S | SLOPE_STEEP_S => Some(CORNER_S),
        SLOPE_E | SLOPE_STEEP_E => Some(CORNER_E),
        SLOPE_N | SLOPE_STEEP_N => Some(CORNER_N),
        _ => None,
    }
}

#[inline]
fn is_non_continuous_foundation(foundation: u8) -> bool {
    foundation == FOUNDATION_STEEP_BOTH
        || (FOUNDATION_HALFTILE_W..=FOUNDATION_HALFTILE_N).contains(&foundation)
}

#[inline]
fn push_track_pass(
    plan: &mut RailTrackDrawPlan,
    track_bits: u8,
    sprite_tileh: u8,
    z_delta: u8,
    halftile_corner: Option<u8>,
) {
    if track_bits == 0 {
        return;
    }
    let pass = RailTrackSpritePass {
        track_bits,
        sprite_tileh,
        z_delta,
        halftile_corner,
    };
    if plan.passes[0].is_none() {
        plan.passes[0] = Some(pass);
    } else if plan.passes[1].is_none() {
        plan.passes[1] = Some(pass);
    } else {
        debug_assert!(false, "DrawTrackBits emitió más de dos pasadas");
    }
}

/// Replica las pasadas de vía de `DrawTrackBits` / `DrawTrackBitsOverlay`.
///
/// A diferencia de [`rail_surface_slope_and_z`], este plan conserva la parte
/// baja de una fundación de medio bloque y describe de forma explícita la
/// pasada superior. Ambas ramas de OpenTTD (rail clásico y railtypes que usan
/// overlay, como monorail/maglev) siguen esta misma división antes de elegir
/// sus sprites.
#[must_use]
pub fn rail_track_draw_plan(tileh: u8, bits: u8) -> RailTrackDrawPlan {
    let track_bits = bits & 0x3F;
    let mut plan = RailTrackDrawPlan {
        passes: [None, None],
    };
    if track_bits == 0 {
        return plan;
    }

    let foundation = rail_foundation_for_trackbits(tileh, track_bits);
    if !is_non_continuous_foundation(foundation) {
        let (surface_tileh, z_delta) = rail_surface_slope_and_z(tileh, track_bits);
        push_track_pass(&mut plan, track_bits, surface_tileh, z_delta, None);
        return plan;
    }

    let halftile_corner = if foundation == FOUNDATION_STEEP_BOTH {
        highest_slope_corner(tileh)
    } else {
        foundation - FOUNDATION_HALFTILE_W
    };
    let lower_track_bits = track_bits & !corner_to_track_bits(halftile_corner);

    // En `SteepBoth`, la pasada inferior se hace después de `SteepLower`, que
    // reduce la pendiente a una esquina elevada y sube una unidad de tile.
    let (lower_tileh, lower_z_delta) = if foundation == FOUNDATION_STEEP_BOTH {
        (
            slope_with_one_corner_raised(halftile_corner),
            u8::from(is_steep_slope(tileh)),
        )
    } else {
        (tileh, 0)
    };
    push_track_pass(
        &mut plan,
        lower_track_bits,
        lower_tileh,
        lower_z_delta,
        None,
    );

    // `DrawTrackBits` no usa la codificación HalftileSlope para elegir el
    // gráfico de arriba. Construye una pendiente de tres esquinas elevadas.
    let upper_tileh = slope_with_three_corners_raised(opposite_corner(halftile_corner));
    push_track_pass(
        &mut plan,
        corner_to_track_bits(halftile_corner),
        upper_tileh,
        lower_z_delta,
        Some(halftile_corner),
    );
    plan
}

#[inline]
fn is_special_rail_foundation(foundation: u8) -> bool {
    (FOUNDATION_RAIL_W..=FOUNDATION_RAIL_N).contains(&foundation)
}

#[inline]
fn foundation_sprite_bases(sprite_block: u8) -> (u32, u32, u32) {
    let block = u32::from(sprite_block.min(3));
    let leveled = if block == 0 {
        FOUNDATION_ORIGINAL_SPRITE_BASE
    } else {
        FOUNDATION_SLOPES_VIRTUAL_BASE + block * FOUNDATION_BLOCK_SIZE
    };
    let inclined =
        FOUNDATION_SLOPES_VIRTUAL_BASE + FOUNDATION_INCLINED_OFFSET + block * FOUNDATION_BLOCK_SIZE;
    let halftile = FOUNDATION_ACTION5_SPRITE_BASE
        + FOUNDATION_HALFTILE_OFFSET
        + block * FOUNDATION_HALFTILE_BLOCK_SIZE;
    (leveled, inclined, halftile)
}

#[inline]
fn push_foundation_sprite(plan: &mut RailFoundationDrawPlan, sprite_id: u32, z_delta: u8) {
    let draw = RailFoundationSpriteDraw { sprite_id, z_delta };
    if plan.sprites[0].is_none() {
        plan.sprites[0] = Some(draw);
    } else if plan.sprites[1].is_none() {
        plan.sprites[1] = Some(draw);
    } else {
        debug_assert!(false, "DrawFoundation emitió más de dos sprites");
    }
}

/// Emite los sprites de una única invocación de `DrawFoundation` y devuelve
/// la pendiente/elevación que quedaría para la siguiente invocación.
#[allow(clippy::too_many_lines)] // Espeja el árbol de casos de `DrawFoundation`.
fn draw_foundation_step(
    plan: &mut RailFoundationDrawPlan,
    tileh: u8,
    foundation: u8,
    sprite_block: u8,
    z_before: u8,
) -> (u8, u8) {
    if foundation == 0 || foundation == FOUNDATION_INVALID {
        return (tileh, 0);
    }

    let (leveled_base, inclined_base, halftile_base) = foundation_sprite_bases(sprite_block);
    if is_steep_slope(tileh) {
        if !is_non_continuous_foundation(foundation) {
            push_foundation_sprite(
                plan,
                leveled_base + u32::from(tileh & !SLOPE_STEEP),
                z_before,
            );
        }

        let highest = highest_slope_corner(tileh);
        let (surface, z_change) = match foundation {
            FOUNDATION_LEVELED => (0, 2),
            FOUNDATION_INCLINED_X => {
                let slope = if highest == CORNER_W || highest == CORNER_S {
                    SLOPE_S | SLOPE_W
                } else {
                    SLOPE_N | SLOPE_E
                };
                (slope, 1)
            }
            FOUNDATION_INCLINED_Y => {
                let slope = if highest == CORNER_S || highest == CORNER_E {
                    SLOPE_S | SLOPE_E
                } else {
                    SLOPE_N | SLOPE_W
                };
                (slope, 1)
            }
            FOUNDATION_STEEP_LOWER => (slope_with_one_corner_raised(highest), 1),
            FOUNDATION_HALFTILE_W..=FOUNDATION_HALFTILE_N => {
                let corner = foundation - FOUNDATION_HALFTILE_W;
                (tileh | SLOPE_HALFTILE | (corner << 6), 0)
            }
            FOUNDATION_STEEP_BOTH => {
                let slope = slope_with_one_corner_raised(highest) | SLOPE_HALFTILE | (highest << 6);
                (slope, 1)
            }
            FOUNDATION_RAIL_W..=FOUNDATION_RAIL_N => {
                let corner = foundation - FOUNDATION_RAIL_W;
                (slope_with_three_corners_raised(opposite_corner(corner)), 0)
            }
            _ => (tileh, 0),
        };
        let z_after = z_before.saturating_add(z_change);

        match foundation {
            FOUNDATION_INCLINED_X | FOUNDATION_INCLINED_Y => {
                let inclined =
                    u32::from(highest) * 2 + u32::from(foundation == FOUNDATION_INCLINED_Y);
                push_foundation_sprite(plan, inclined_base + inclined, z_after);
            }
            FOUNDATION_LEVELED => {
                push_foundation_sprite(
                    plan,
                    leveled_base + u32::from(slope_with_one_corner_raised(highest)),
                    z_after,
                );
            }
            FOUNDATION_STEEP_LOWER => {}
            FOUNDATION_HALFTILE_W..=FOUNDATION_HALFTILE_N => {
                push_foundation_sprite(plan, halftile_base + u32::from(highest), z_after);
            }
            _ => {}
        }
        return (surface, z_change);
    }

    match foundation {
        FOUNDATION_LEVELED => {
            push_foundation_sprite(plan, leveled_base + u32::from(tileh), z_before);
        }
        FOUNDATION_HALFTILE_W..=FOUNDATION_HALFTILE_N => {
            let corner = foundation - FOUNDATION_HALFTILE_W;
            push_foundation_sprite(plan, halftile_base + u32::from(corner), z_before);
        }
        foundation if is_special_rail_foundation(foundation) => {
            let corner = foundation - FOUNDATION_RAIL_W;
            let sprite_id = if tileh == (SLOPE_N | SLOPE_S) || tileh == (SLOPE_W | SLOPE_E) {
                leveled_base + u32::from(slope_with_three_corners_raised(corner))
            } else {
                let along_x = tileh == (SLOPE_S | SLOPE_W) || tileh == (SLOPE_N | SLOPE_E);
                inclined_base + u32::from(corner) * 2 + u32::from(along_x)
            };
            push_foundation_sprite(plan, sprite_id, z_before);
        }
        FOUNDATION_INCLINED_X | FOUNDATION_INCLINED_Y => {
            let highest = highest_slope_corner(tileh);
            let inclined = u32::from(highest) * 2 + u32::from(foundation == FOUNDATION_INCLINED_Y);
            push_foundation_sprite(plan, inclined_base + inclined, z_before);
        }
        _ => {}
    }

    let (surface, z_change) = match foundation {
        FOUNDATION_LEVELED => (0, 1),
        FOUNDATION_HALFTILE_W..=FOUNDATION_HALFTILE_N => {
            let corner = foundation - FOUNDATION_HALFTILE_W;
            (tileh | SLOPE_HALFTILE | (corner << 6), 0)
        }
        foundation if is_special_rail_foundation(foundation) => {
            let corner = foundation - FOUNDATION_RAIL_W;
            (slope_with_three_corners_raised(opposite_corner(corner)), 0)
        }
        FOUNDATION_INCLINED_X => {
            let highest = highest_slope_corner(tileh);
            let slope = if highest == CORNER_W || highest == CORNER_S {
                SLOPE_S | SLOPE_W
            } else {
                SLOPE_N | SLOPE_E
            };
            (slope, 0)
        }
        FOUNDATION_INCLINED_Y => {
            let highest = highest_slope_corner(tileh);
            let slope = if highest == CORNER_S || highest == CORNER_E {
                SLOPE_S | SLOPE_E
            } else {
                SLOPE_N | SLOPE_W
            };
            (slope, 0)
        }
        _ => (tileh, 0),
    };
    (surface, z_change)
}

/// Replica una llamada a `DrawFoundation`.
///
/// La representación se llamó originalmente `RailFoundationDrawPlan` porque
/// nació al portar `DrawTrackBits`, pero OpenTTD usa exactamente el mismo
/// dibujador para puentes, estaciones y otras fundaciones.  Mantener el plan
/// genérico evita que una rampa de puente seleccione una tabla distinta de
/// sprites que una vía para la misma fundación.
///
/// `sprite_block` es el bloque 0..3 elegido por `HasFoundationNW/NE`: no se
/// infiere acá porque depende de las fundaciones de las teselas vecinas.
#[must_use]
pub fn foundation_draw_plan(tileh: u8, foundation: u8, sprite_block: u8) -> RailFoundationDrawPlan {
    let mut plan = RailFoundationDrawPlan {
        sprites: [None, None],
        surface_tileh: tileh,
        surface_z_delta: 0,
    };
    if foundation == 0 || foundation == FOUNDATION_INVALID {
        return plan;
    }

    let (surface, z_delta) = if foundation == FOUNDATION_STEEP_BOTH {
        // OpenTTD dibuja primero la parte inferior y después el medio tile
        // superior, con la pendiente ya transformada por SteepLower.
        let (lower_slope, lower_delta) =
            draw_foundation_step(&mut plan, tileh, FOUNDATION_STEEP_LOWER, sprite_block, 0);
        let upper = FOUNDATION_HALFTILE_W + highest_slope_corner(tileh);
        let (surface, upper_delta) =
            draw_foundation_step(&mut plan, lower_slope, upper, sprite_block, lower_delta);
        (surface, lower_delta.saturating_add(upper_delta))
    } else {
        draw_foundation_step(&mut plan, tileh, foundation, sprite_block, 0)
    };
    plan.surface_tileh = surface;
    plan.surface_z_delta = z_delta;
    plan
}

/// Replica las llamadas a `DrawFoundation` que rodean a `DrawTrackBits`.
///
/// Es el adaptador ferroviario de [`foundation_draw_plan`].
#[must_use]
pub fn rail_foundation_draw_plan(tileh: u8, bits: u8, sprite_block: u8) -> RailFoundationDrawPlan {
    foundation_draw_plan(
        tileh,
        rail_foundation_for_trackbits(tileh, bits),
        sprite_block,
    )
}

/// `true` si los `TrackBits` pueden colocarse en `tileh` (fundación distinta de inválida).
#[must_use]
pub fn rail_trackbits_valid_on_slope(tileh: u8, bits: u8) -> bool {
    rail_foundation_for_trackbits(tileh, bits) != FOUNDATION_INVALID
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{Map, TileCoord, tile_slope_and_z};

    #[test]
    fn flat_allows_all_trackbits() {
        assert!(rail_trackbits_valid_on_slope(0, 0x3F));
        assert!(rail_trackbits_valid_on_slope(0, TRACK_BIT_X));
        assert!(rail_trackbits_valid_on_slope(0, TRACK_BIT_X | TRACK_BIT_Y));
    }

    #[test]
    fn ew_ridge_allows_diagonal_with_leveled_foundation() {
        // `SLOPE_EW` (5): sin fundación no cabe nada; con fundación nivelada sí (`GetRailFoundation`).
        assert!(rail_trackbits_valid_on_slope(5, TRACK_BIT_X));
        assert!(rail_trackbits_valid_on_slope(5, TRACK_BIT_HORZ));
    }

    #[test]
    fn sw_slope_allows_x_and_y_with_foundation() {
        assert!(rail_trackbits_valid_on_slope(3, TRACK_BIT_X));
        assert!(rail_trackbits_valid_on_slope(3, TRACK_BIT_Y));
    }

    #[test]
    fn inclined_foundation_changes_the_render_slope() {
        // SLOPE_E + TRACK_X usa FOUNDATION_INCLINED_X. OpenTTD transforma la
        // superficie a SLOPE_NE antes de seleccionar `_track_sloped_sprites`.
        assert_eq!(
            rail_foundation_for_trackbits(SLOPE_E, TRACK_BIT_X),
            FOUNDATION_INCLINED_X
        );
        assert_eq!(
            rail_surface_slope_and_z(SLOPE_E, TRACK_BIT_X),
            (SLOPE_N | SLOPE_E, 0)
        );

        // Una fundación nivelada sube un nivel de tile y deja la superficie
        // plana, por lo que la vía usa el sprite plano.
        assert_eq!(
            rail_surface_slope_and_z(SLOPE_E | SLOPE_W, TRACK_BIT_X),
            (0, 1)
        );
    }

    #[test]
    fn visual_plan_matches_surface_transform_for_every_track_mask() {
        for tileh in 0..32 {
            for bits in 0..64 {
                let plan = rail_foundation_draw_plan(tileh, bits, 0);
                assert_eq!(
                    (plan.surface_tileh, plan.surface_z_delta),
                    rail_surface_slope_and_z(tileh, bits),
                    "tileh={tileh}, bits={bits:#04x}"
                );
            }
        }
    }

    #[test]
    fn halftile_track_plan_uses_openttd_fake_three_corner_slope() {
        // Kale_TitleGame (158,65): SLOPE_N + TRACK_UPPER. OpenTTD descarta
        // la pasada inferior y elige la pendiente falsa SLOPE_NWE (=13),
        // cuyo offset de vía clásica es 19: sprite 1030.
        assert_eq!(
            rail_track_draw_plan(SLOPE_N, TRACK_BIT_UPPER),
            RailTrackDrawPlan {
                passes: [
                    Some(RailTrackSpritePass {
                        track_bits: TRACK_BIT_UPPER,
                        sprite_tileh: SLOPE_N | SLOPE_W | SLOPE_E,
                        z_delta: 0,
                        halftile_corner: Some(CORNER_N),
                    }),
                    None,
                ],
            }
        );

        // Otros tres casos presentes en la misma partida: cada esquina usa
        // la pendiente falsa opuesta, no la pendiente cruda de la tesela.
        assert_eq!(
            rail_track_draw_plan(SLOPE_W, TRACK_BIT_LEFT).passes[0],
            Some(RailTrackSpritePass {
                track_bits: TRACK_BIT_LEFT,
                sprite_tileh: SLOPE_N | SLOPE_W | SLOPE_S,
                z_delta: 0,
                halftile_corner: Some(CORNER_W),
            })
        );
        assert_eq!(
            rail_track_draw_plan(SLOPE_E, TRACK_BIT_RIGHT).passes[0],
            Some(RailTrackSpritePass {
                track_bits: TRACK_BIT_RIGHT,
                sprite_tileh: SLOPE_N | SLOPE_S | SLOPE_E,
                z_delta: 0,
                halftile_corner: Some(CORNER_E),
            })
        );
        assert_eq!(
            rail_track_draw_plan(SLOPE_S, TRACK_BIT_LOWER).passes[0],
            Some(RailTrackSpritePass {
                track_bits: TRACK_BIT_LOWER,
                sprite_tileh: SLOPE_W | SLOPE_S | SLOPE_E,
                z_delta: 0,
                halftile_corner: Some(CORNER_S),
            })
        );
    }

    #[test]
    fn halftile_track_plan_preserves_the_lower_pass_before_the_high_one() {
        // Kale_TitleGame (160,65): SLOPE_S con UPPER|LOWER. `DrawTrackBits`
        // primero conserva UPPER sobre la pendiente cruda, y sólo después de
        // `DrawFoundation(Halftile(S))` dibuja LOWER sobre SLOPE_WSE. Unir
        // ambas en una sola pasada hacía que la fundación quedara delante del
        // tramo bajo y rompía el orden observable del draw proc.
        assert_eq!(
            rail_track_draw_plan(SLOPE_S, TRACK_BIT_HORZ),
            RailTrackDrawPlan {
                passes: [
                    Some(RailTrackSpritePass {
                        track_bits: TRACK_BIT_UPPER,
                        sprite_tileh: SLOPE_S,
                        z_delta: 0,
                        halftile_corner: None,
                    }),
                    Some(RailTrackSpritePass {
                        track_bits: TRACK_BIT_LOWER,
                        sprite_tileh: SLOPE_W | SLOPE_S | SLOPE_E,
                        z_delta: 0,
                        halftile_corner: Some(CORNER_S),
                    }),
                ],
            }
        );
    }

    #[test]
    fn steep_both_track_plan_keeps_lower_and_upper_passes() {
        // SLOPE_STEEP_W + TRACK_VERT: la vía LEFT queda en la mitad alta y
        // RIGHT se dibuja sobre la pasada SteepLower. `DrawFoundation` eleva
        // ambas una unidad antes de la parte alta.
        assert_eq!(
            rail_foundation_for_trackbits(SLOPE_STEEP_W, TRACK_BIT_VERT),
            FOUNDATION_STEEP_BOTH
        );
        assert_eq!(
            rail_track_draw_plan(SLOPE_STEEP_W, TRACK_BIT_VERT),
            RailTrackDrawPlan {
                passes: [
                    Some(RailTrackSpritePass {
                        track_bits: TRACK_BIT_RIGHT,
                        sprite_tileh: SLOPE_W,
                        z_delta: 1,
                        halftile_corner: None,
                    }),
                    Some(RailTrackSpritePass {
                        track_bits: TRACK_BIT_LEFT,
                        sprite_tileh: SLOPE_N | SLOPE_W | SLOPE_S,
                        z_delta: 1,
                        halftile_corner: Some(CORNER_W),
                    }),
                ],
            }
        );
    }

    #[test]
    fn inclined_sprite_uses_the_same_extra_foundation_as_openttd_trace() {
        // Kale_TitleGame (42,42): SLOPE_E + TRACK_X, bloque 2 (sin pared
        // NE) sale como sprite 5461 en `DrawFoundation` de OpenTTD 15.3.
        let plan = rail_foundation_draw_plan(SLOPE_E, TRACK_BIT_X, 2);
        assert_eq!(
            plan.sprites,
            [
                Some(RailFoundationSpriteDraw {
                    sprite_id: 5461,
                    z_delta: 0,
                }),
                None,
            ]
        );
        assert_eq!(plan.surface_tileh, SLOPE_N | SLOPE_E);
    }

    #[test]
    fn leveled_block_zero_keeps_the_original_foundation_sprite() {
        let plan = rail_foundation_draw_plan(SLOPE_W | SLOPE_E, TRACK_BIT_X, 0);
        assert_eq!(
            plan.sprites[0],
            Some(RailFoundationSpriteDraw {
                sprite_id: FOUNDATION_ORIGINAL_SPRITE_BASE + u32::from(SLOPE_W | SLOPE_E),
                z_delta: 0,
            })
        );
        assert_eq!(plan.surface_tileh, 0);
        assert_eq!(plan.surface_z_delta, 1);
    }

    #[test]
    fn generic_leveled_plan_matches_kale_bridge_foundation_trace() {
        // Kale_TitleGame (92,148): `GetBridgeFoundation(SLOPE_NES, Axis::X)`
        // devuelve Leveled. Con ambas paredes ocultas OpenTTD selecciona el
        // bloque Action5 3: sprite 5478, antes de la rampa de carretera 2450.
        let plan = foundation_draw_plan(SLOPE_N | SLOPE_E | SLOPE_S, FOUNDATION_LEVELED, 3);
        assert_eq!(
            plan.sprites,
            [
                Some(RailFoundationSpriteDraw {
                    sprite_id: 5478,
                    z_delta: 0,
                }),
                None,
            ]
        );
        assert_eq!(plan.surface_tileh, 0);
        assert_eq!(plan.surface_z_delta, 1);
    }

    #[test]
    fn w_corner_rejects_horz_track() {
        // `SLOPE_W` (1): solo RIGHT sin fundación; HORZ no es válido ni con fundación nivelada.
        assert!(!rail_trackbits_valid_on_slope(1, TRACK_BIT_HORZ));
        assert!(rail_trackbits_valid_on_slope(1, TRACK_BIT_RIGHT));
    }

    #[test]
    fn computed_tileh_matches_openrtd_sw() {
        let mut map = Map::new_flat(4, 4, 1);
        let c = TileCoord::new(1, 1);
        map.set_height(c, 1).unwrap();
        map.set_height(TileCoord::new(2, 1), 2).unwrap();
        map.set_height(TileCoord::new(1, 2), 1).unwrap();
        map.set_height(TileCoord::new(2, 2), 2).unwrap();
        let (tileh, _) = tile_slope_and_z(&map, c).unwrap();
        assert_eq!(tileh, 3);
        assert!(rail_trackbits_valid_on_slope(tileh, TRACK_BIT_X));
    }

    #[test]
    fn steep_slope_only_inclined_diagonals_or_halftile_corner() {
        assert!(rail_trackbits_valid_on_slope(SLOPE_STEEP_W, TRACK_BIT_X));
        assert!(rail_trackbits_valid_on_slope(SLOPE_STEEP_W, TRACK_BIT_Y));
        assert!(!rail_trackbits_valid_on_slope(
            SLOPE_STEEP_W,
            TRACK_BIT_HORZ
        ));
        assert!(rail_trackbits_valid_on_slope(SLOPE_STEEP_W, TRACK_BIT_LEFT));
    }
}
