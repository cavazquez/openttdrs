//! Procedimientos `draw_proc` 1–5 de OpenTTD (`IndustryDraw*` en `industry_cmd.cpp`).

#[path = "industry_draw_proc_generated.rs"]
mod generated;

pub use generated::{
    COAL_PLANT_SPARKS, DRAW_INDUSTRY_SPEC1, DRAW_TILE_PROC1, INDUSTRY_ANIM_OFFS_BUBBLES,
    INDUSTRY_ANIM_OFFS_TOFFEE, INDUSTRY_ANIM_OFFS_TOYS, INDUSTRY_DRAW_PROC,
    INDUSTRY_DRAW_PROC_SPRITE_IDS, SPR_IT_BUBBLE_GENERATOR_BUBBLE, SPR_IT_BUBBLE_GENERATOR_SPRING,
    SPR_IT_POWER_PLANT_TRANSFORMERS, SPR_IT_SUGAR_MINE_CLOUDS, SPR_IT_SUGAR_MINE_PILE,
    SPR_IT_SUGAR_MINE_SIEVE, SPR_IT_TOFFEE_QUARRY_SHOVEL, SPR_IT_TOFFEE_QUARRY_TOFFEE,
    SPR_IT_TOY_FACTORY_CLAY, SPR_IT_TOY_FACTORY_ROBOT, SPR_IT_TOY_FACTORY_STAMP,
    SPR_IT_TOY_FACTORY_STAMP_HOLDER,
};

use super::industry_construction_stage_from_tile;

/// `draw_proc` (1–5) para esta fila de `_industry_draw_tile_data`.
#[must_use]
pub fn industry_draw_proc(gfx: u16, construction_stage: usize) -> u8 {
    industry_draw_proc_extended(gfx, construction_stage)
}

/// Igual que upstream: `dits->draw_proc` con fallback para gfx ≥131.
#[must_use]
pub fn industry_draw_proc_extended(gfx: u16, construction_stage: usize) -> u8 {
    let stage = construction_stage.min(3);
    if usize::from(gfx) < INDUSTRY_DRAW_PROC.len() / 4 {
        return INDUSTRY_DRAW_PROC[usize::from(gfx) * 4 + stage];
    }
    match gfx {
        143 if stage == 3 => 4,
        162 if stage >= 1 => 3,
        165 => 2,
        174 => 1,
        _ => 0,
    }
}

/// `draw_proc` activo para bytes de tesela (`m1`).
#[must_use]
pub fn industry_draw_proc_for_tile(gfx: u16, m1: u8) -> u8 {
    industry_draw_proc(gfx, industry_construction_stage_from_tile(m1))
}

/// Frame de animación completo (`GetAnimationFrame`), no solo `& 3`.
#[must_use]
pub fn industry_draw_proc_anim_frame(m3hi: u8) -> u8 {
    m3hi
}

/// Capas extra a dibujar para un `draw_proc` y frame dados.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawProcLayer {
    pub sprite_id: u32,
    pub dx: i32,
    pub dy: i32,
}

const TOFFEE_INVALID: u8 = 255;

/// Overlays dinámicos (cambian con `m3hi`).
#[must_use]
pub fn industry_draw_proc_dynamic_layers(proc: u8, m1: u8, frame: u8) -> Vec<DrawProcLayer> {
    if proc == 0 || proc > 5 {
        return Vec::new();
    }
    let completed = m1 & 0x80 != 0;
    match proc {
        1 if completed => sugar_mine_layers(frame),
        2 => toffee_quarry_layers(completed, frame),
        3 => bubble_generator_layers(completed, frame),
        4 => toy_factory_layers(frame),
        5 if completed => coal_plant_sparks_layers(frame),
        _ => Vec::new(),
    }
}

fn sugar_mine_layers(frame: u8) -> Vec<DrawProcLayer> {
    let Some(d) = DRAW_INDUSTRY_SPEC1.get(frame as usize) else {
        return Vec::new();
    };
    let mut out = vec![DrawProcLayer {
        sprite_id: SPR_IT_SUGAR_MINE_SIEVE + u32::from(d.image_1),
        dx: d.x,
        dy: 0,
    }];
    if d.image_2 != 0 {
        out.push(DrawProcLayer {
            sprite_id: SPR_IT_SUGAR_MINE_CLOUDS + u32::from(d.image_2) - 1,
            dx: 8,
            dy: 41,
        });
    }
    if d.image_3 != 0 {
        let idx = (d.image_3 - 1) as usize;
        if let Some(c) = DRAW_TILE_PROC1.get(idx) {
            out.push(DrawProcLayer {
                sprite_id: SPR_IT_SUGAR_MINE_PILE + u32::from(d.image_3) - 1,
                dx: i32::from(c.x),
                dy: i32::from(c.y),
            });
        }
    }
    out
}

fn toffee_quarry_layers(completed: bool, frame: u8) -> Vec<DrawProcLayer> {
    let mut x_off = 0i32;
    if completed {
        let raw = INDUSTRY_ANIM_OFFS_TOFFEE
            .get(frame as usize)
            .copied()
            .unwrap_or(TOFFEE_INVALID);
        if raw != TOFFEE_INVALID {
            x_off = i32::from(raw);
        }
    }
    vec![
        DrawProcLayer {
            sprite_id: SPR_IT_TOFFEE_QUARRY_SHOVEL,
            dx: 22 - x_off,
            dy: 24 + x_off,
        },
        DrawProcLayer {
            sprite_id: SPR_IT_TOFFEE_QUARRY_TOFFEE,
            dx: 6,
            dy: 14,
        },
    ]
}

fn bubble_generator_layers(completed: bool, frame: u8) -> Vec<DrawProcLayer> {
    let mut out = Vec::with_capacity(2);
    if completed {
        let dy = INDUSTRY_ANIM_OFFS_BUBBLES
            .get(frame as usize)
            .copied()
            .unwrap_or(68);
        out.push(DrawProcLayer {
            sprite_id: SPR_IT_BUBBLE_GENERATOR_BUBBLE,
            dx: 5,
            dy: i32::from(dy),
        });
    }
    out.push(DrawProcLayer {
        sprite_id: SPR_IT_BUBBLE_GENERATOR_SPRING,
        dx: 3,
        dy: 67,
    });
    out
}

fn toy_factory_layers(frame: u8) -> Vec<DrawProcLayer> {
    let Some(d) = INDUSTRY_ANIM_OFFS_TOYS.get(frame as usize) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(4);
    if d.image_1 != TOFFEE_INVALID {
        out.push(DrawProcLayer {
            sprite_id: SPR_IT_TOY_FACTORY_CLAY,
            dx: d.x,
            dy: 96 + i32::from(d.image_1),
        });
    }
    if d.image_2 != TOFFEE_INVALID {
        out.push(DrawProcLayer {
            sprite_id: SPR_IT_TOY_FACTORY_ROBOT,
            dx: 16 - i32::from(d.image_2) * 2,
            dy: 100 + i32::from(d.image_2),
        });
    }
    out.push(DrawProcLayer {
        sprite_id: SPR_IT_TOY_FACTORY_STAMP,
        dx: 7,
        dy: i32::from(d.image_3),
    });
    out.push(DrawProcLayer {
        sprite_id: SPR_IT_TOY_FACTORY_STAMP_HOLDER,
        dx: 0,
        dy: 42,
    });
    out
}

fn coal_plant_sparks_layers(frame: u8) -> Vec<DrawProcLayer> {
    if frame == 0 || frame >= 7 {
        return Vec::new();
    }
    let idx = (frame - 1) as usize;
    let Some(c) = COAL_PLANT_SPARKS.get(idx) else {
        return Vec::new();
    };
    vec![DrawProcLayer {
        sprite_id: SPR_IT_POWER_PLANT_TRANSFORMERS + u32::from(frame),
        dx: i32::from(c.x),
        dy: i32::from(c.y),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_lookup_in_table_and_extended() {
        assert_eq!(industry_draw_proc(10, 3), 5);
        assert_eq!(industry_draw_proc(174, 0), 1);
        assert_eq!(industry_draw_proc(0, 3), 0);
    }

    #[test]
    fn sparks_only_when_frame_nonzero() {
        assert!(coal_plant_sparks_layers(0).is_empty());
        assert_eq!(coal_plant_sparks_layers(3).len(), 1);
    }

    #[test]
    fn bubble_spring_always_present() {
        let layers = bubble_generator_layers(false, 0);
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].sprite_id, SPR_IT_BUBBLE_GENERATOR_SPRING);
    }
}
