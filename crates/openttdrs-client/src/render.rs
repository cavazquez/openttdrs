//! Tipos y helpers pequeños para construir la capa visual del mapa.

use std::collections::HashMap;

use bevy::prelude::*;
use openttdrs_core::{Map, Tile, TileCoord, TileKind};

use crate::iso::{TILE_HALF_H, iso, tile_pos, tile_pos_half, tile_slope_and_min_z, wang_hash};
use crate::sprites::{
    HOUSE_DRAW_DATA, INDUSTRY_GFX_DATA, house_sprite_filename, rail_sprite_ids_for_preload,
};

/// Sesgo en la componente Z de **solo** el agua animada (sin sprite `shore_*`).
/// El orden de dibujo usa `(tx+ty)`; el mar al **este/sur** tiene suma mayor y acaba
/// encima del borde costero del vecino NO/NE → sierra y rectángulos azules oscuros.
const FLAT_WATER_LAYER_FRAC: f32 = -0.014;

/// Marca los tiles de agua para la animación por ondas.
/// Almacena fases discretas por tile para emular el ciclado de paleta
/// (dark water 5 pasos + glitter 15 pasos).
#[derive(Component)]
pub(crate) struct WaterTile {
    pub(crate) dark_phase: u8,
    pub(crate) glitter_phase: u8,
}

/// Teselas de suelo, vías, vehículos, etc.: se despawnan al recargar JSON (F9).
#[derive(Component)]
pub(crate) struct MapVisualLayer;

pub(crate) struct WorldAssets {
    pub(crate) grass: Handle<Image>,
    pub(crate) rough: Handle<Image>,
    pub(crate) grass_slopes: Vec<Handle<Image>>,
    pub(crate) rough_slopes: Vec<Handle<Image>>,
    pub(crate) water: Handle<Image>,
    pub(crate) shore: Vec<Handle<Image>>,
    pub(crate) lighthouse: Handle<Image>,
    pub(crate) transmitter: Handle<Image>,
    pub(crate) road_flat: Vec<Handle<Image>>,
    pub(crate) rail: HashMap<u32, Handle<Image>>,
    pub(crate) station_grounds: Vec<Handle<Image>>,
    pub(crate) houses: HashMap<u32, Handle<Image>>,
    pub(crate) trees: [Handle<Image>; 3],
    pub(crate) industries: HashMap<u32, Handle<Image>>,
}

impl WorldAssets {
    pub(crate) fn load(asset_server: &AssetServer) -> Self {
        let grass = asset_server.load::<Image>("opengfx/tiles/grass.png");
        let rough = asset_server.load::<Image>("opengfx/tiles/grass_rough.png");
        let grass_slopes = (1u8..=14)
            .map(|tileh| {
                asset_server
                    .load::<Image>(format!("opengfx/tiles/terrain_grass_slope_{tileh:02}.png"))
            })
            .collect();
        let rough_slopes = (1u8..=14)
            .map(|tileh| {
                asset_server
                    .load::<Image>(format!("opengfx/tiles/terrain_rough_slope_{tileh:02}.png"))
            })
            .collect();
        let water = asset_server.load::<Image>("opengfx/tiles/water.png");
        let shore = (0..8)
            .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/shore_{i}.png")))
            .collect();
        let lighthouse = asset_server.load::<Image>("opengfx/tiles/object_lighthouse.png");
        let transmitter = asset_server.load::<Image>("opengfx/tiles/object_transmitter.png");
        let road_flat = (0..19)
            .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/road_flat_{i:02}.png")))
            .collect();
        let rail = rail_sprite_ids_for_preload()
            .into_iter()
            .map(|id| {
                (
                    id,
                    asset_server.load::<Image>(format!("opengfx/tiles/rail_{id}.png")),
                )
            })
            .collect();
        let station_grounds = (0..4)
            .map(|i| asset_server.load::<Image>(format!("opengfx/tiles/truck_stop_ground_{i}.png")))
            .collect();

        let mut houses = HashMap::new();
        for spec in &HOUSE_DRAW_DATA {
            for &sid in &[spec.s1, spec.s2] {
                if sid != 0 {
                    let fname = house_sprite_filename(sid);
                    houses.entry(sid).or_insert_with(|| {
                        asset_server.load::<Image>(format!("opengfx/tiles/{fname}"))
                    });
                }
            }
        }

        let trees = [
            asset_server.load::<Image>("opengfx/tiles/tree_00.png"),
            asset_server.load::<Image>("opengfx/tiles/tree_07.png"),
            asset_server.load::<Image>("opengfx/tiles/tree_14.png"),
        ];

        let mut industries = HashMap::new();
        for entry in &INDUSTRY_GFX_DATA {
            if entry.sprite_id != 0 {
                industries.entry(entry.sprite_id).or_insert_with(|| {
                    asset_server
                        .load::<Image>(format!("opengfx/tiles/industry_{}.png", entry.sprite_id))
                });
            }
        }

        Self {
            grass,
            rough,
            grass_slopes,
            rough_slopes,
            water,
            shore,
            lighthouse,
            transmitter,
            road_flat,
            rail,
            station_grounds,
            houses,
            trees,
            industries,
        }
    }
}

/// `true` si algún vecino ortogonal no es agua ni vacío (borde mar/tierra o río).
///
/// Los exports `.ottdmap` a veces dejan `m5=0` en toda el agua y se pierde
/// `WaterTileType::Coast` en bits 4–7; sin esto solo se pinta agua plana en la orilla.
fn water_tile_touches_land(map: &Map, tx: u32, ty: u32, mw: u32, mh: u32) -> bool {
    let is_land = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= mw as i32 || y >= mh as i32 {
            return false;
        }
        map.get(TileCoord::new(x, y))
            .is_some_and(|t| t.kind != TileKind::Water && t.kind != TileKind::Void)
    };
    let x = tx as i32;
    let y = ty as i32;
    is_land(x - 1, y) || is_land(x + 1, y) || is_land(x, y - 1) || is_land(x, y + 1)
}

#[derive(Clone, Copy)]
pub(crate) struct TileRenderInfo {
    pub(crate) tileh: u8,
    pub(crate) base_z: u8,
    pub(crate) use_shore: bool,
}

pub(crate) struct RenderGrid {
    width: u32,
    tiles: Vec<TileRenderInfo>,
}

impl RenderGrid {
    pub(crate) fn from_map(map: &Map, mw: u32, mh: u32) -> Self {
        let mut tiles = vec![
            TileRenderInfo {
                tileh: 0,
                base_z: 0,
                use_shore: false,
            };
            (mw * mh) as usize
        ];

        for ty in 0..mh {
            for tx in 0..mw {
                let idx = (ty * mw + tx) as usize;
                let (tileh, base_z) = tile_slope_and_min_z(map, tx, ty);
                let c = TileCoord::new(tx as i32, ty as i32);
                let tile = map.get(c);
                let kind = tile.map_or(TileKind::Grass, |t| t.kind);
                let use_shore = if kind == TileKind::Water {
                    let m5_w = tile.map_or(0u8, |t| t.m5);
                    let water_tile_type = (m5_w >> 4) & 0x0F;
                    water_tile_type == 1
                        || (water_tile_type == 0 && water_tile_touches_land(map, tx, ty, mw, mh))
                } else {
                    false
                };
                tiles[idx] = TileRenderInfo {
                    tileh,
                    base_z,
                    use_shore,
                };
            }
        }

        Self { width: mw, tiles }
    }

    fn get(&self, tx: u32, ty: u32) -> TileRenderInfo {
        self.tiles[(ty * self.width + tx) as usize]
    }
}

pub(crate) struct TileRenderContext {
    pub(crate) tx: u32,
    pub(crate) ty: u32,
    pub(crate) coord: TileCoord,
    pub(crate) tile: Option<Tile>,
    pub(crate) kind: TileKind,
    pub(crate) info: TileRenderInfo,
    pub(crate) iso_pos: Vec2,
}

impl TileRenderContext {
    pub(crate) fn new(map: &Map, grid: &RenderGrid, tx: u32, ty: u32) -> Self {
        let coord = TileCoord::new(tx as i32, ty as i32);
        let tile = map.get(coord);
        let kind = tile.map_or(TileKind::Grass, |t| t.kind);
        Self {
            tx,
            ty,
            coord,
            tile,
            kind,
            info: grid.get(tx, ty),
            iso_pos: iso(tx as i32, ty as i32),
        }
    }

    pub(crate) fn tx_i32(&self) -> i32 {
        self.tx as i32
    }

    pub(crate) fn ty_i32(&self) -> i32 {
        self.ty as i32
    }
}

pub(crate) fn sloped_or_flat_image(
    tileh: u8,
    flat: &Handle<Image>,
    slopes: &[Handle<Image>],
) -> Handle<Image> {
    if tileh == 0 {
        flat.clone()
    } else {
        slopes[tileh as usize - 1].clone()
    }
}

pub(crate) fn spawn_ground_sprite(
    commands: &mut Commands,
    image: Handle<Image>,
    color: Color,
    ctx: &TileRenderContext,
    half_h: f32,
) {
    commands.spawn((
        MapVisualLayer,
        Sprite {
            image,
            color,
            ..default()
        },
        Transform::from_translation(tile_pos_half(
            ctx.tx_i32(),
            ctx.ty_i32(),
            ctx.info.base_z,
            0.0,
            half_h,
        )),
    ));
}

fn water_phases(tx: u32, ty: u32) -> WaterTile {
    WaterTile {
        dark_phase: ((tx + 2 * ty).rem_euclid(5)) as u8,
        glitter_phase: (wang_hash(tx, ty, 0xA9FE) % 15) as u8,
    }
}

pub(crate) fn push_water_sprite(
    batch_water: &mut Vec<(WaterTile, Sprite, Transform)>,
    h_water: &Handle<Image>,
    ctx: &TileRenderContext,
) {
    batch_water.push((
        water_phases(ctx.tx, ctx.ty),
        Sprite {
            image: h_water.clone(),
            color: Color::WHITE,
            ..default()
        },
        Transform::from_translation(tile_pos(
            ctx.tx_i32(),
            ctx.ty_i32(),
            ctx.info.base_z,
            FLAT_WATER_LAYER_FRAC,
        )),
    ));
}

pub(crate) fn spawn_coast_debug_label(
    commands: &mut Commands,
    ctx: &TileRenderContext,
    raw: u8,
    tileh: u8,
    shore_index: usize,
) {
    let label = format!("r{raw}/t{tileh}/s{shore_index}");
    commands.spawn((
        MapVisualLayer,
        Text2d::new(label),
        TextFont {
            font_size: 9.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.95, 0.4)),
        Transform::from_translation(Vec3::new(
            ctx.iso_pos.x - 18.0,
            ctx.iso_pos.y - TILE_HALF_H + f32::from(ctx.info.base_z) * 8.0 - 3.0,
            (ctx.tx + ctx.ty) as f32 * 0.01 + f32::from(ctx.info.base_z) * 0.001 + 0.95,
        )),
    ));
}
