//! Helpers compartidos para caches NewGRF in-world (#135).

mod fingerprint;
mod image_factory;

use openttdrs_core::map::SPR_FLAT_WATER_TILE;
use openttdrs_core::newgrf_sprites::{ResolvedTileLayout, ResolvedTileLayoutSprite};
use openttdrs_core::{DecodedSprite, TWOCC_ACTION5_SLOT_COUNT, TWOCC_PALETTE_BASE};

use crate::render::{AtlasSprite, WorldAssets};

pub(crate) use fingerprint::runtime_fingerprint;
pub(crate) use image_factory::{
    DecodedSpriteImagePolicy, decoded_sprite_image, decoded_sprite_image_with_twocc_map,
    decoded_tile_layout_image_with_palette_and_twocc_map, tile_layout_entry_is_hidden,
    tile_layout_sprite_color_with_palette,
};

/// Obtiene la tabla Action5 `0x0A` asociada a un `PaletteID` 2CC.
///
/// El rango es deliberadamente cerrado al tamaño nativo de la tabla: una
/// paleta fuera de `SPR_2CCMAP_BASE..SPR_2CCMAP_BASE+256` debe seguir usando
/// el remapeo vanilla o la política de la textura, nunca indexar una entrada
/// arbitraria del runtime.
#[must_use]
pub(crate) fn twocc_map_for_palette(
    maps: &[Option<DecodedSprite>],
    palette_id: u16,
) -> Option<DecodedSprite> {
    let slot = palette_id.checked_sub(TWOCC_PALETTE_BASE)?;
    if slot >= TWOCC_ACTION5_SLOT_COUNT as u16 {
        return None;
    }
    maps.get(usize::from(slot))
        .and_then(Option::as_ref)
        .cloned()
}

/// Geometry of a normal-size baseset sprite as emitted by `DrawGroundSprite`.
///
/// The PNG is cropped to its visible rectangle, so its pixel dimensions alone
/// are not enough to position it. These values mirror the normal 8bpp NFO
/// rows in `ogfx1_base.nfo`; the same logical geometry is used by the active
/// OpenGFX profile when the atlas is built.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DirectTileLayoutGroundGeometry {
    width: f32,
    height: f32,
    x_offs: f32,
    y_offs: f32,
}

const STANDARD_TERRAIN_GROUND_GEOMETRY: [DirectTileLayoutGroundGeometry; 19] = [
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 23.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 23.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 23.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 23.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 39.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 39.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 39.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 39.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 47.0,
        x_offs: -31.0,
        y_offs: -16.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 15.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
];

/// Water uses the same ground draw contract for the first nine offsets, then
/// switches to partial edge sprites. Keeping those final anchors is what
/// prevents a direct water slope from becoming a misplaced 64×31 tile.
const WATER_GROUND_GEOMETRY: [DirectTileLayoutGroundGeometry; 19] = [
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 31.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 23.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 39.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 23.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 39.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 23.0,
        x_offs: -31.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 39.0,
        x_offs: -31.0,
        y_offs: -8.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 64.0,
        x_offs: -61.0,
        y_offs: -48.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 64.0,
        height: 64.0,
        x_offs: -1.0,
        y_offs: -47.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 32.0,
        height: 53.0,
        x_offs: -29.0,
        y_offs: -37.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 32.0,
        height: 53.0,
        x_offs: -1.0,
        y_offs: -36.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 14.0,
        height: 13.0,
        x_offs: -31.0,
        y_offs: 2.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 14.0,
        height: 13.0,
        x_offs: 19.0,
        y_offs: 3.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 1.0,
        height: 1.0,
        x_offs: 0.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 20.0,
        height: 20.0,
        x_offs: 0.0,
        y_offs: 0.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 1.0,
        height: 1.0,
        x_offs: 4.0,
        y_offs: 3.0,
    },
    DirectTileLayoutGroundGeometry {
        width: 3.0,
        height: 3.0,
        x_offs: -1.0,
        y_offs: -1.0,
    },
];

fn direct_tile_layout_ground_geometry(sprite_id: u16) -> Option<DirectTileLayoutGroundGeometry> {
    let (geometry, offset) = match sprite_id {
        3924..=3999 => (
            &STANDARD_TERRAIN_GROUND_GEOMETRY,
            usize::from((sprite_id - 3924) % 19),
        ),
        4000..=4018 => (
            &STANDARD_TERRAIN_GROUND_GEOMETRY,
            usize::from(sprite_id - 4000),
        ),
        4019..=4022 => (&STANDARD_TERRAIN_GROUND_GEOMETRY, 0),
        4023..=4060 => (
            &STANDARD_TERRAIN_GROUND_GEOMETRY,
            usize::from((sprite_id - 4023) % 19),
        ),
        4061..=4079 => (&WATER_GROUND_GEOMETRY, usize::from(sprite_id - 4061)),
        4493..=4568 => (
            &STANDARD_TERRAIN_GROUND_GEOMETRY,
            usize::from((sprite_id - 4493) % 19),
        ),
        _ => return None,
    };
    geometry.get(offset).copied()
}

/// Direct baseset references that have a known atlas asset and NFO anchor.
/// These ranges cover the four grass densities, rough/rocky terrain, water,
/// and the four snow/desert densities. Any modifier or palette still forces
/// the complete layout fallback in `tile_layout_is_renderable`.
fn direct_tile_layout_ground_sprite_is_supported(sprite_id: u16) -> bool {
    direct_tile_layout_ground_geometry(sprite_id).is_some()
        || direct_tile_layout_industry_ground_geometry(sprite_id).is_some()
        || direct_tile_layout_house_ground_geometry(sprite_id).is_some()
}

/// Busca la geometría del suelo propio de una industria (`s1`) en todas sus
/// etapas. Hay suelos planos y piezas parciales de industrias custom; sólo se
/// admite un ID cuando todas sus apariciones tienen el mismo ancla NFO.
fn direct_tile_layout_industry_ground_geometry(
    sprite_id: u16,
) -> Option<DirectTileLayoutGroundGeometry> {
    if sprite_id == 0 {
        return None;
    }
    let sprite_id = u32::from(sprite_id);
    let mut geometry = None;
    for entry in crate::sprites::INDUSTRY_GFX_DATA
        .iter()
        .filter(|entry| entry.ground_sprite_id == sprite_id)
    {
        let candidate = DirectTileLayoutGroundGeometry {
            width: entry.ground_w,
            height: entry.ground_h,
            x_offs: entry.ground_xrel,
            y_offs: entry.ground_yrel,
        };
        if let Some(previous) = geometry {
            if previous != candidate {
                return None;
            }
        } else {
            geometry = Some(candidate);
        }
    }
    geometry
}

/// Busca la geometría del suelo propio de una casa (`s1`) en todas las vistas
/// y etapas. Los IDs vanilla de terreno siguen resolviéndose por sus rangos;
/// esta tabla cubre las fachadas, patios y piezas parciales que usan un
/// sprite de casa como base. Sólo se acepta una geometría consistente.
fn direct_tile_layout_house_ground_geometry(
    sprite_id: u16,
) -> Option<DirectTileLayoutGroundGeometry> {
    if sprite_id == 0 {
        return None;
    }
    let sprite_id = u32::from(sprite_id);
    let mut geometry = None;
    for spec in crate::sprites::HOUSE_DRAW_DATA
        .iter()
        .filter(|spec| spec.s1 == sprite_id)
    {
        let candidate = DirectTileLayoutGroundGeometry {
            width: spec.s1_w,
            height: spec.s1_h,
            x_offs: spec.s1_xrel,
            y_offs: spec.s1_yrel,
        };
        if let Some(previous) = geometry {
            if previous != candidate {
                return None;
            }
        } else {
            geometry = Some(candidate);
        }
    }
    geometry
}

/// Busca la geometría del overlay de industria en todas sus etapas. Un mismo
/// sprite puede repetirse cuando sólo cambia el suelo o la etapa; si el atlas
/// publicado tuviera dos anclas distintas para el mismo ID, no se debe elegir
/// una al azar y desplazar la secuencia `BUILD`.
fn direct_tile_layout_industry_geometry(sprite_id: u16) -> Option<DirectTileLayoutGroundGeometry> {
    if sprite_id == 0 {
        return None;
    }
    let sprite_id = u32::from(sprite_id);
    let mut geometry = None;
    for entry in crate::sprites::INDUSTRY_GFX_DATA
        .iter()
        .filter(|entry| entry.sprite_id == sprite_id)
    {
        let candidate = DirectTileLayoutGroundGeometry {
            width: entry.w,
            height: entry.h,
            x_offs: entry.xrel,
            y_offs: entry.yrel,
        };
        if let Some(previous) = geometry {
            if previous != candidate {
                return None;
            }
        } else {
            geometry = Some(candidate);
        }
    }
    geometry
}

/// Busca la geometría del overlay de casa (`s2`) en todas las vistas y etapas
/// de `_town_draw_tile_data`. Las paletas de compañía se resuelven aparte;
/// aquí sólo habilitamos la referencia directa sin recolour explícito.
fn direct_tile_layout_house_geometry(sprite_id: u16) -> Option<DirectTileLayoutGroundGeometry> {
    if sprite_id == 0 {
        return None;
    }
    let sprite_id = u32::from(sprite_id);
    let mut geometry = None;
    for spec in crate::sprites::HOUSE_DRAW_DATA
        .iter()
        .filter(|spec| spec.s2 == sprite_id)
    {
        let candidate = DirectTileLayoutGroundGeometry {
            width: spec.s2_w,
            height: spec.s2_h,
            x_offs: spec.s2_xrel,
            y_offs: spec.s2_yrel,
        };
        if let Some(previous) = geometry {
            if previous != candidate {
                return None;
            }
        } else {
            geometry = Some(candidate);
        }
    }
    geometry
}

/// Geometría NFO de las referencias directas compartidas que `object_land.h`
/// usa dentro del namespace de objetos. Estos IDs también pueden aparecer en
/// tablas de estación (por ejemplo, el transmisor 2601); por eso el resolver
/// genérico conserva sólo este subconjunto y el consumidor de objetos tiene
/// un resolver contextual.
fn direct_tile_layout_common_object_geometry(
    sprite_id: u16,
) -> Option<DirectTileLayoutGroundGeometry> {
    match sprite_id {
        1420 => Some(DirectTileLayoutGroundGeometry {
            width: 64.0,
            height: 31.0,
            x_offs: -31.0,
            y_offs: 0.0,
        }), // SPR_CONCRETE_GROUND
        2601 => Some(DirectTileLayoutGroundGeometry {
            width: 54.0,
            height: 94.0,
            x_offs: -26.0,
            y_offs: -80.0,
        }), // SPR_TRANSMITTER
        2602 => Some(DirectTileLayoutGroundGeometry {
            width: 21.0,
            height: 64.0,
            x_offs: -9.0,
            y_offs: -52.0,
        }), // SPR_LIGHTHOUSE
        2632 => Some(DirectTileLayoutGroundGeometry {
            width: 60.0,
            height: 45.0,
            x_offs: -30.0,
            y_offs: -42.0,
        }), // SPR_STATUE_COMPANY
        4790 => Some(DirectTileLayoutGroundGeometry {
            width: 32.0,
            height: 48.0,
            x_offs: -16.0,
            y_offs: -40.0,
        }), // SPR_BOUGHT_LAND
        _ => None,
    }
}

/// Geometría NFO de todas las referencias directas de `object_land.h`.
///
/// Las sedes (`2603..2631`) comparten rango numérico con sprites de otros
/// consumidores del baseset. Por eso no se agregan al resolver global: sólo
/// `DrawNewObjectTile` puede seleccionar este namespace sin ambigüedad.
fn direct_tile_layout_object_geometry(sprite_id: u16) -> Option<DirectTileLayoutGroundGeometry> {
    direct_tile_layout_common_object_geometry(sprite_id).or_else(|| {
        crate::sprites::company_hq_sprite_meta(u32::from(sprite_id)).map(|meta| {
            DirectTileLayoutGroundGeometry {
                width: meta.width,
                height: meta.height,
                x_offs: meta.x_offs,
                y_offs: meta.y_offs,
            }
        })
    })
}

fn direct_tile_layout_object_atlas(sprite_id: u16, assets: &WorldAssets) -> Option<AtlasSprite> {
    match sprite_id {
        1420 => Some(assets.object_concrete.clone()),
        2601 => Some(assets.transmitter.clone()),
        2602 => Some(assets.lighthouse.clone()),
        2632 => Some(assets.company_statue.clone()),
        4790 => Some(assets.bought_land.clone()),
        _ => {
            let hq_index =
                u32::from(sprite_id).checked_sub(crate::sprites::COMPANY_HQ_SPRITE_BASE)?;
            if hq_index >= crate::sprites::COMPANY_HQ_SPRITE_COUNT as u32 {
                return None;
            }
            assets.hq.get(usize::try_from(hq_index).ok()?).cloned()
        }
    }
}

/// Geometría NFO de los sprites vanilla que `DrawRoadStop` puede publicar en
/// un `TileLayout`. El rango de bus/truck y las tiras drive-through viven en
/// el namespace de estaciones viales; no se agregan al resolver global porque
/// sus SpriteID también pueden aparecer en otros bancos del baseset.
fn direct_tile_layout_road_stop_geometry(sprite_id: u16) -> Option<DirectTileLayoutGroundGeometry> {
    let sprite_id = u32::from(sprite_id);
    if (2692..=2695).contains(&sprite_id) || (2708..=2711).contains(&sprite_id) {
        return Some(DirectTileLayoutGroundGeometry {
            width: 64.0,
            height: 31.0,
            x_offs: -31.0,
            y_offs: 0.0,
        });
    }

    for class in [
        crate::sprites::StationTileClass::Bus,
        crate::sprites::StationTileClass::Truck,
    ] {
        for direction in 0..4 {
            if let Some(layer) = crate::sprites::road_stop_build_layers(class, direction)
                .iter()
                .find(|layer| layer.sprite_id == sprite_id)
            {
                return Some(DirectTileLayoutGroundGeometry {
                    width: layer.w,
                    height: layer.h,
                    x_offs: layer.x_offs,
                    y_offs: layer.y_offs,
                });
            }
        }
        for orientation in [4u8, 5u8] {
            if let Some(layer) = crate::sprites::road_stop_drive_through_layers(class, orientation)
                .iter()
                .find(|layer| layer.sprite_id == sprite_id)
            {
                return Some(DirectTileLayoutGroundGeometry {
                    width: layer.w,
                    height: layer.h,
                    x_offs: layer.x_offs,
                    y_offs: layer.y_offs,
                });
            }
        }
    }
    None
}

/// Geometría NFO de las dos capas por eje que `DrawTile_Station` publica para
/// un waypoint vial vanilla. Este namespace no comparte las tablas de
/// marquesinas bus/truck: los mismos layouts pueden seleccionar los cuatro
/// sprites de postes `6141..6144` directamente.
fn direct_tile_layout_road_waypoint_geometry(
    sprite_id: u16,
) -> Option<DirectTileLayoutGroundGeometry> {
    let sprite_id = u32::from(sprite_id);
    (0..2).find_map(|axis| {
        crate::sprites::road_waypoint_build_layers(axis)
            .iter()
            .find(|layer| layer.sprite_id == sprite_id)
            .map(|layer| DirectTileLayoutGroundGeometry {
                width: layer.w,
                height: layer.h,
                x_offs: layer.x_offs,
                y_offs: layer.y_offs,
            })
    })
}

fn direct_tile_layout_road_waypoint_atlas(
    sprite_id: u16,
    assets: &WorldAssets,
) -> Option<AtlasSprite> {
    let index = crate::sprites::road_waypoint_sprite_index(u32::from(sprite_id))?;
    assets.road_waypoint.get(index).cloned()
}

fn direct_tile_layout_road_stop_atlas(sprite_id: u16, assets: &WorldAssets) -> Option<AtlasSprite> {
    let sprite_id = u32::from(sprite_id);
    if (2692..=2695).contains(&sprite_id) {
        return assets
            .bus_stop_grounds
            .get(usize::try_from(sprite_id - 2692).ok()?)
            .cloned();
    }
    if (2708..=2711).contains(&sprite_id) {
        return assets
            .station_grounds
            .get(usize::try_from(sprite_id - 2708).ok()?)
            .cloned();
    }

    for class in [
        crate::sprites::StationTileClass::Bus,
        crate::sprites::StationTileClass::Truck,
    ] {
        for direction in 0..4 {
            let layers = crate::sprites::road_stop_build_layers(class, direction);
            if let Some(layer_index) = layers.iter().position(|layer| layer.sprite_id == sprite_id)
            {
                return match class {
                    crate::sprites::StationTileClass::Bus => {
                        Some(assets.bus_stop_builds[direction][layer_index].clone())
                    }
                    crate::sprites::StationTileClass::Truck => {
                        Some(assets.truck_stop_builds[direction][layer_index].clone())
                    }
                    _ => None,
                };
            }
        }
        for axis in 0..2 {
            let orientation = 4 + axis as u8;
            let layers = crate::sprites::road_stop_drive_through_layers(class, orientation);
            if let Some(layer_index) = layers.iter().position(|layer| layer.sprite_id == sprite_id)
            {
                return match class {
                    crate::sprites::StationTileClass::Bus => {
                        Some(assets.bus_stop_drive_through[axis][layer_index].clone())
                    }
                    crate::sprites::StationTileClass::Truck => {
                        Some(assets.truck_stop_drive_through[axis][layer_index].clone())
                    }
                    _ => None,
                };
            }
        }
    }
    None
}

/// Geometría NFO de un sprite vanilla que puede aparecer en una secuencia
/// `BUILD`. Además del terreno, las tablas de estación rail y airport ya
/// conservan el tamaño y el ancla de cada sprite; no es correcto tratarlos
/// como un rombo plano sólo porque vienen de un `TileLayout` directo.
fn direct_tile_layout_sequence_geometry(sprite_id: u16) -> Option<DirectTileLayoutGroundGeometry> {
    direct_tile_layout_ground_geometry(sprite_id)
        .or_else(|| direct_tile_layout_industry_ground_geometry(sprite_id))
        .or_else(|| direct_tile_layout_house_ground_geometry(sprite_id))
        .or_else(|| direct_tile_layout_industry_geometry(sprite_id))
        .or_else(|| direct_tile_layout_house_geometry(sprite_id))
        .or_else(|| {
            crate::sprites::rail_station_sprite_meta(u32::from(sprite_id)).map(
                |(width, height, x_offs, y_offs)| DirectTileLayoutGroundGeometry {
                    width,
                    height,
                    x_offs,
                    y_offs,
                },
            )
        })
        .or_else(|| {
            crate::sprites::airport_station_sprite_for_id(u32::from(sprite_id)).map(|sprite| {
                DirectTileLayoutGroundGeometry {
                    width: sprite.w,
                    height: sprite.h,
                    x_offs: sprite.x_offs,
                    y_offs: sprite.y_offs,
                }
            })
        })
        .or_else(|| direct_tile_layout_common_object_geometry(sprite_id))
}

fn direct_tile_layout_sequence_sprite_is_supported(sprite_id: u16) -> bool {
    direct_tile_layout_sequence_geometry(sprite_id).is_some()
}

fn direct_tile_layout_road_stop_sequence_sprite_is_supported(sprite_id: u16) -> bool {
    direct_tile_layout_sequence_sprite_is_supported(sprite_id)
        || direct_tile_layout_road_stop_geometry(sprite_id).is_some()
}

fn direct_tile_layout_road_stop_ground_sprite_is_supported(sprite_id: u16) -> bool {
    direct_tile_layout_ground_sprite_is_supported(sprite_id)
        || direct_tile_layout_road_stop_geometry(sprite_id).is_some()
}

fn direct_tile_layout_road_waypoint_sequence_sprite_is_supported(sprite_id: u16) -> bool {
    direct_tile_layout_sequence_sprite_is_supported(sprite_id)
        || direct_tile_layout_road_waypoint_geometry(sprite_id).is_some()
}

fn direct_tile_layout_road_waypoint_ground_sprite_is_supported(sprite_id: u16) -> bool {
    direct_tile_layout_ground_sprite_is_supported(sprite_id)
        || direct_tile_layout_road_waypoint_geometry(sprite_id).is_some()
}

fn direct_tile_layout_object_sequence_sprite_is_supported(sprite_id: u16) -> bool {
    direct_tile_layout_sequence_sprite_is_supported(sprite_id)
        || direct_tile_layout_object_geometry(sprite_id).is_some()
}

fn direct_tile_layout_object_ground_sprite_is_supported(sprite_id: u16) -> bool {
    direct_tile_layout_ground_sprite_is_supported(sprite_id)
        || direct_tile_layout_object_geometry(sprite_id).is_some()
}

/// Base-sprite data that a TileLayout renderer needs in addition to the atlas
/// rect. `AtlasSprite` deliberately has no NFO offset, so resolving only
/// audited IDs prevents a direct reference from silently using a wrong anchor.
#[derive(Clone)]
pub(crate) struct DirectTileLayoutGround {
    pub(crate) atlas: AtlasSprite,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) x_offs: f32,
    pub(crate) y_offs: f32,
}

/// Whether every TileLayout entry can be emitted by the current compact
/// renderer. Action1 entries are fully decoded; a direct base reference is
/// accepted only for an audited ground sprite with a known atlas asset and NFO
/// geometry, which is also safe when the entry is a BUILD parent or child.
#[must_use]
pub(crate) fn tile_layout_is_renderable(layout: &ResolvedTileLayout) -> bool {
    if !layout.complete
        || layout.sequence.iter().any(|entry| {
            entry.action1_sprite().is_none()
                && !entry.base_sprite_id().is_some_and(|id| {
                    entry.sprite_modifiers == 0
                        && entry.direct_palette == 0
                        && direct_tile_layout_sequence_sprite_is_supported(id)
                })
        })
    {
        return false;
    }
    match layout.ground.as_ref() {
        None => true,
        Some(ground) => {
            ground.action1_sprite().is_some()
                || ground.base_sprite_id().is_some_and(|id| {
                    ground.sprite_modifiers == 0
                        && ground.direct_palette == 0
                        && direct_tile_layout_ground_sprite_is_supported(id)
                })
        }
    }
}

/// Variante contextual para `DrawNewObjectTile`.
///
/// El contrato global no admite todavía el rango HQ porque `2603..2631`
/// colisiona con sprites de túneles, puertos y herramientas en otros
/// namespaces. Un layout de objeto sí puede resolverlo de forma inequívoca,
/// así que amplía únicamente sus referencias directas sin cambiar la
/// decisión de estaciones, casas o industrias.
#[must_use]
pub(crate) fn tile_layout_is_object_renderable(layout: &ResolvedTileLayout) -> bool {
    if !layout.complete
        || layout.sequence.iter().any(|entry| {
            entry.action1_sprite().is_none()
                && !entry.base_sprite_id().is_some_and(|id| {
                    entry.sprite_modifiers == 0
                        && entry.direct_palette == 0
                        && direct_tile_layout_object_sequence_sprite_is_supported(id)
                })
        })
    {
        return false;
    }
    match layout.ground.as_ref() {
        None => true,
        Some(ground) => {
            ground.action1_sprite().is_some()
                || ground.base_sprite_id().is_some_and(|id| {
                    ground.sprite_modifiers == 0
                        && ground.direct_palette == 0
                        && direct_tile_layout_object_ground_sprite_is_supported(id)
                })
        }
    }
}

/// Variante contextual para `DrawRoadStop`.
///
/// Las piezas vanilla de bus/truck y las tiras drive-through se resuelven
/// contra sus tablas de estación vial. Mantener este contrato separado evita
/// que un ID numéricamente coincidente de industria, vía o herramienta cambie
/// de atlas en los otros consumidores de `TileLayout`.
#[must_use]
pub(crate) fn tile_layout_is_road_stop_renderable(layout: &ResolvedTileLayout) -> bool {
    if !layout.complete
        || layout.sequence.iter().any(|entry| {
            entry.action1_sprite().is_none()
                && !entry.base_sprite_id().is_some_and(|id| {
                    entry.sprite_modifiers == 0
                        && entry.direct_palette == 0
                        && direct_tile_layout_road_stop_sequence_sprite_is_supported(id)
                })
        })
    {
        return false;
    }
    match layout.ground.as_ref() {
        None => true,
        Some(ground) => {
            ground.action1_sprite().is_some()
                || ground.base_sprite_id().is_some_and(|id| {
                    ground.sprite_modifiers == 0
                        && ground.direct_palette == 0
                        && direct_tile_layout_road_stop_ground_sprite_is_supported(id)
                })
        }
    }
}

/// Variante contextual para `DrawTile_Station` cuando la estación es un
/// waypoint vial. Sus postes usan un atlas distinto al de bus/truck, aunque
/// compartan el mismo contrato Action2/TileLayout y los mismos slots de caché.
#[must_use]
pub(crate) fn tile_layout_is_road_waypoint_renderable(layout: &ResolvedTileLayout) -> bool {
    if !layout.complete
        || layout.sequence.iter().any(|entry| {
            entry.action1_sprite().is_none()
                && !entry.base_sprite_id().is_some_and(|id| {
                    entry.sprite_modifiers == 0
                        && entry.direct_palette == 0
                        && direct_tile_layout_road_waypoint_sequence_sprite_is_supported(id)
                })
        })
    {
        return false;
    }
    match layout.ground.as_ref() {
        None => true,
        Some(ground) => {
            ground.action1_sprite().is_some()
                || ground.base_sprite_id().is_some_and(|id| {
                    ground.sprite_modifiers == 0
                        && ground.direct_palette == 0
                        && direct_tile_layout_road_waypoint_ground_sprite_is_supported(id)
                })
        }
    }
}

/// Resolves the texture and audited NFO geometry of a supported direct base
/// ground. Action1 sprites stay in the per-feature NewGRF cache instead.
#[must_use]
pub(crate) fn direct_tile_layout_ground(
    ground: &ResolvedTileLayoutSprite,
    assets: &WorldAssets,
) -> Option<DirectTileLayoutGround> {
    if ground.action1_sprite().is_some()
        || ground.sprite_modifiers != 0
        || ground.direct_palette != 0
    {
        return None;
    }
    let sprite_id = ground.base_sprite_id()?;
    let atlas = match sprite_id {
        3924 => assets.industries.get(&3924)?.clone(), // SPR_FLAT_BARE_LAND
        3925..=3942 => assets.grass_density[0][usize::from(sprite_id - 3924)].clone(),
        3943 => assets.grass_density[1][0].clone(), // SPR_FLAT_1_THIRD_GRASS_TILE
        3944..=3961 => assets.grass_density[1][usize::from(sprite_id - 3943)].clone(),
        3962 => assets.grass_density[2][0].clone(), // SPR_FLAT_2_THIRD_GRASS_TILE
        3963..=3980 => assets.grass_density[2][usize::from(sprite_id - 3962)].clone(),
        3981 => assets.grass.clone(), // SPR_FLAT_GRASS_TILE
        3982..=3999 => assets
            .grass_slopes
            .get(usize::from(sprite_id - 3982))?
            .clone(),
        4000 => assets.rough_flat[0].clone(), // SPR_FLAT_ROUGH_LAND
        4001..=4018 => assets
            .rough_slopes
            .get(usize::from(sprite_id - 4001))?
            .clone(),
        4019 => assets.rough_flat[1].clone(), // SPR_FLAT_ROUGH_LAND_1
        4020 => assets.rough_flat[2].clone(), // SPR_FLAT_ROUGH_LAND_2
        4021 => assets.rough_flat[3].clone(), // SPR_FLAT_ROUGH_LAND_3
        4022 => assets.rough_flat[4].clone(), // SPR_FLAT_ROUGH_LAND_4
        4023..=4041 => assets.rocky[0][usize::from(sprite_id - 4023)].clone(),
        4042..=4060 => assets.rocky[1][usize::from(sprite_id - 4042)].clone(),
        id if id == SPR_FLAT_WATER_TILE as u16 => assets.water.clone(),
        4062..=4079 => assets.water_slopes[usize::from(sprite_id - 4061)].clone(),
        4493 => assets.snow_desert[0][0].clone(), // SPR_FLAT_1_QUART_SNOW_DESERT_TILE
        4494..=4511 => assets.snow_desert[0][usize::from(sprite_id - 4493)].clone(),
        4512 => assets.snow_desert[1][0].clone(), // SPR_FLAT_2_QUART_SNOW_DESERT_TILE
        4513..=4530 => assets.snow_desert[1][usize::from(sprite_id - 4512)].clone(),
        4531 => assets.snow_desert[2][0].clone(), // SPR_FLAT_3_QUART_SNOW_DESERT_TILE
        4532..=4549 => assets.snow_desert[2][usize::from(sprite_id - 4531)].clone(),
        4550 => assets.snow_desert[3][0].clone(), // SPR_FLAT_SNOW_DESERT_TILE
        4551..=4568 => assets.snow_desert[3][usize::from(sprite_id - 4550)].clone(),
        _ => assets
            .industries
            .get(&u32::from(sprite_id))
            .or_else(|| assets.houses.get(&u32::from(sprite_id)))?
            .clone(),
    };
    let geometry = direct_tile_layout_ground_geometry(sprite_id)
        .or_else(|| direct_tile_layout_industry_ground_geometry(sprite_id))
        .or_else(|| direct_tile_layout_house_ground_geometry(sprite_id))?;
    Some(DirectTileLayoutGround {
        atlas,
        width: geometry.width,
        height: geometry.height,
        x_offs: geometry.x_offs,
        y_offs: geometry.y_offs,
    })
}

/// Resolves a direct base reference used by a `BUILD` sequence. Terrain keeps
/// using the ground resolver; station sprites then use their own atlas maps
/// and exact NFO dimensions/anchors. A missing atlas entry remains a caller
/// fallback instead of borrowing a visually similar sprite.
#[must_use]
pub(crate) fn direct_tile_layout_sequence(
    layer: &ResolvedTileLayoutSprite,
    assets: &WorldAssets,
) -> Option<DirectTileLayoutGround> {
    if let Some(ground) = direct_tile_layout_ground(layer, assets) {
        return Some(ground);
    }
    if layer.action1_sprite().is_some() || layer.sprite_modifiers != 0 || layer.direct_palette != 0
    {
        return None;
    }
    let sprite_id = layer.base_sprite_id()?;
    let geometry = direct_tile_layout_sequence_geometry(sprite_id)?;
    let atlas = assets
        .rail
        .get(&u32::from(sprite_id))
        .cloned()
        .or_else(|| assets.industries.get(&u32::from(sprite_id)).cloned())
        .or_else(|| assets.houses.get(&u32::from(sprite_id)).cloned())
        .or_else(|| assets.airport_station_sprite(u32::from(sprite_id)).cloned())?;
    Some(DirectTileLayoutGround {
        atlas,
        width: geometry.width,
        height: geometry.height,
        x_offs: geometry.x_offs,
        y_offs: geometry.y_offs,
    })
}

/// Resuelve una referencia directa de una secuencia `BUILD` desde el
/// contexto `DrawNewObjectTile`. Los IDs vanilla comparten un espacio global
/// con estaciones e industrias, así que el resolver genérico no puede elegir
/// siempre la textura correcta: 2601, por ejemplo, tiene una entrada de
/// aeropuerto distinta. Para objetos se prioriza el atlas y la geometría de
/// `object_land.h`; el resto conserva el resolver compartido.
#[must_use]
pub(crate) fn direct_tile_layout_object_sequence(
    layer: &ResolvedTileLayoutSprite,
    assets: &WorldAssets,
) -> Option<DirectTileLayoutGround> {
    if layer.action1_sprite().is_some() || layer.sprite_modifiers != 0 || layer.direct_palette != 0
    {
        return None;
    }
    if let Some(sprite_id) = layer.base_sprite_id()
        && let Some(geometry) = direct_tile_layout_object_geometry(sprite_id)
        && let Some(atlas) = direct_tile_layout_object_atlas(sprite_id, assets)
    {
        return Some(DirectTileLayoutGround {
            atlas,
            width: geometry.width,
            height: geometry.height,
            x_offs: geometry.x_offs,
            y_offs: geometry.y_offs,
        });
    }
    direct_tile_layout_sequence(layer, assets)
}

/// Resuelve una referencia directa desde el namespace de `RoadStops`.
/// Primero se consultan las tablas vanilla de bus/truck y drive-through; el
/// resto conserva el resolver global para terreno, estaciones e industrias.
#[must_use]
pub(crate) fn direct_tile_layout_road_stop_sequence(
    layer: &ResolvedTileLayoutSprite,
    assets: &WorldAssets,
) -> Option<DirectTileLayoutGround> {
    if layer.action1_sprite().is_some() || layer.sprite_modifiers != 0 || layer.direct_palette != 0
    {
        return None;
    }
    if let Some(sprite_id) = layer.base_sprite_id()
        && let Some(geometry) = direct_tile_layout_road_stop_geometry(sprite_id)
        && let Some(atlas) = direct_tile_layout_road_stop_atlas(sprite_id, assets)
    {
        return Some(DirectTileLayoutGround {
            atlas,
            width: geometry.width,
            height: geometry.height,
            x_offs: geometry.x_offs,
            y_offs: geometry.y_offs,
        });
    }
    direct_tile_layout_sequence(layer, assets)
}

/// Resuelve una referencia directa desde el namespace de `RoadWaypoint`.
/// Primero se consultan las tablas de los cuatro postes vanilla; sólo si el
/// ID no pertenece a ellas se reutiliza el resolver compartido de terreno,
/// estaciones e industrias.
#[must_use]
pub(crate) fn direct_tile_layout_road_waypoint_sequence(
    layer: &ResolvedTileLayoutSprite,
    assets: &WorldAssets,
) -> Option<DirectTileLayoutGround> {
    if layer.action1_sprite().is_some() || layer.sprite_modifiers != 0 || layer.direct_palette != 0
    {
        return None;
    }
    if let Some(sprite_id) = layer.base_sprite_id()
        && let Some(geometry) = direct_tile_layout_road_waypoint_geometry(sprite_id)
        && let Some(atlas) = direct_tile_layout_road_waypoint_atlas(sprite_id, assets)
    {
        return Some(DirectTileLayoutGround {
            atlas,
            width: geometry.width,
            height: geometry.height,
            x_offs: geometry.x_offs,
            y_offs: geometry.y_offs,
        });
    }
    direct_tile_layout_sequence(layer, assets)
}

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
    /// Variables `CanalScopeResolver` usadas por pendientes, bordes y
    /// callbacks de `Canals`: altura, terreno, conectividad, random y
    /// parámetros del callback actual.
    pub const CANAL: &[u8] = &[0x0C, 0x10, 0x18, 0x80, 0x81, 0x82, 0x83];
    /// Dominio de vehículo conservado para las regresiones unitarias del
    /// fingerprint histórico; el renderer ya no usa un fingerprint de contexto
    /// como identidad del asset.
    #[cfg(test)]
    pub const TRAIN: &[u8] = &[0x10, 0x40, 0x47, 0x43, 0x5F, 0xB2, 0xB4, 0xC8];
}

#[cfg(test)]
mod tests {
    use super::*;
    use openttdrs_core::DecodedSprite;

    fn action1_sprite() -> ResolvedTileLayoutSprite {
        ResolvedTileLayoutSprite {
            sprite: Some(DecodedSprite {
                width: 1,
                height: 1,
                x_offs: 0,
                y_offs: 0,
                rgba: vec![0, 0, 0, 0],
                mask: Vec::new(),
            }),
            base_sprite: None,
            sprite_modifiers: 0,
            direct_palette: 0,
            origin: [0, 0, 0],
            extent: [1, 1, 1],
        }
    }

    #[test]
    fn direct_base_ground_and_build_are_limited_to_audited_ids() {
        let direct_ground = ResolvedTileLayoutSprite {
            sprite: None,
            base_sprite: Some(3981),
            sprite_modifiers: 0,
            direct_palette: 0,
            origin: [0, 0, 0],
            extent: [0, 0, 0],
        };
        let layout = ResolvedTileLayout {
            ground: Some(direct_ground.clone()),
            sequence: vec![action1_sprite()],
            complete: true,
        };
        assert!(tile_layout_is_renderable(&layout));

        let mut unsupported_ground = layout.clone();
        unsupported_ground
            .ground
            .as_mut()
            .expect("ground")
            .base_sprite = Some(4080);
        assert!(!tile_layout_is_renderable(&unsupported_ground));

        for sprite_id in (3924u16..=4079).chain(4493u16..=4568) {
            let mut supported = layout.clone();
            supported.ground.as_mut().expect("ground").base_sprite = Some(sprite_id);
            assert!(
                tile_layout_is_renderable(&supported),
                "sprite de terreno vanilla {sprite_id} debe conservarse como ground"
            );
        }
        for sprite_id in [1424, 2022, 2269, 4471, 4721, 4769] {
            let mut supported = layout.clone();
            supported.ground.as_mut().expect("ground").base_sprite = Some(sprite_id);
            assert!(
                tile_layout_is_renderable(&supported),
                "sprite de suelo industrial {sprite_id} debe conservarse como ground"
            );
        }

        let mut direct_palette = layout.clone();
        direct_palette
            .ground
            .as_mut()
            .expect("ground")
            .direct_palette = 791;
        assert!(!tile_layout_is_renderable(&direct_palette));

        let mut direct_build = layout;
        direct_build.sequence[0].sprite = None;
        direct_build.sequence[0].base_sprite = Some(3981);
        assert!(tile_layout_is_renderable(&direct_build));
        direct_build.sequence[0].base_sprite = Some(4062);
        assert!(tile_layout_is_renderable(&direct_build));
        direct_build.sequence[0].base_sprite = Some(4080);
        assert!(!tile_layout_is_renderable(&direct_build));
    }

    #[test]
    fn direct_base_build_accepts_station_namespaces_with_nfo_geometry() {
        let direct_ground = ResolvedTileLayoutSprite {
            sprite: None,
            base_sprite: Some(3981),
            sprite_modifiers: 0,
            direct_palette: 0,
            origin: [0, 0, 0],
            extent: [0, 0, 0],
        };
        let mut layout = ResolvedTileLayout {
            ground: Some(direct_ground),
            sequence: vec![action1_sprite()],
            complete: true,
        };

        for sprite_id in [
            1069, 1083, 1151, 1233, 4974, 2633, 2651, 2668, 4982, 5966, 2011, 2047, 1421, 1430,
        ] {
            layout.sequence[0].sprite = None;
            layout.sequence[0].base_sprite = Some(sprite_id);
            assert!(
                tile_layout_is_renderable(&layout),
                "sprite de estación vanilla {sprite_id} debe conservarse como BUILD"
            );
        }
        for sprite_id in [1419, 2000, 2692, 4983, 5969] {
            layout.sequence[0].base_sprite = Some(sprite_id);
            assert!(
                !tile_layout_is_renderable(&layout),
                "sprite fuera del catálogo de estación {sprite_id} debe usar fallback"
            );
        }
    }

    #[test]
    fn direct_base_ground_uses_nfo_geometry_for_land_and_water_slopes() {
        assert_eq!(
            direct_tile_layout_ground_geometry(3982),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 31.0,
                x_offs: -31.0,
                y_offs: 0.0,
            })
        );
        assert_eq!(
            direct_tile_layout_ground_geometry(3996),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 47.0,
                x_offs: -31.0,
                y_offs: -16.0,
            })
        );
        assert_eq!(
            direct_tile_layout_ground_geometry(4064),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 23.0,
                x_offs: -31.0,
                y_offs: 0.0,
            })
        );
        assert_eq!(
            direct_tile_layout_ground_geometry(4070),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 64.0,
                x_offs: -61.0,
                y_offs: -48.0,
            })
        );
        assert_eq!(
            direct_tile_layout_ground_geometry(4079),
            Some(DirectTileLayoutGroundGeometry {
                width: 3.0,
                height: 3.0,
                x_offs: -1.0,
                y_offs: -1.0,
            })
        );
        assert_eq!(direct_tile_layout_ground_geometry(4080), None);
        assert_eq!(direct_tile_layout_ground_geometry(4492), None);
        assert_eq!(direct_tile_layout_ground_geometry(4569), None);
    }

    #[test]
    fn direct_base_build_uses_station_nfo_geometry() {
        assert_eq!(
            direct_tile_layout_sequence_geometry(1069),
            Some(DirectTileLayoutGroundGeometry {
                width: 42.0,
                height: 26.0,
                x_offs: -9.0,
                y_offs: -6.0,
            })
        );
        assert_eq!(
            direct_tile_layout_sequence_geometry(2651),
            Some(DirectTileLayoutGroundGeometry {
                width: 42.0,
                height: 79.0,
                x_offs: -19.0,
                y_offs: -60.0,
            })
        );
        assert_eq!(
            direct_tile_layout_sequence_geometry(3981),
            direct_tile_layout_ground_geometry(3981)
        );
        assert_eq!(direct_tile_layout_sequence_geometry(2692), None);
    }

    #[test]
    fn direct_road_waypoint_layout_uses_its_own_namespace_and_geometry() {
        let direct_ground = ResolvedTileLayoutSprite {
            sprite: None,
            base_sprite: Some(3981),
            sprite_modifiers: 0,
            direct_palette: 0,
            origin: [0, 0, 0],
            extent: [1, 1, 1],
        };
        let mut layout = ResolvedTileLayout {
            ground: Some(direct_ground),
            sequence: vec![action1_sprite()],
            complete: true,
        };
        let expected = [
            (6141, 64.0, 40.0, -5.0, -22.0),
            (6142, 64.0, 40.0, -31.0, -9.0),
            (6143, 64.0, 35.0, -31.0, -4.0),
            (6144, 64.0, 35.0, -57.0, -17.0),
        ];

        for (sprite_id, width, height, x_offs, y_offs) in expected {
            assert_eq!(
                direct_tile_layout_road_waypoint_geometry(sprite_id),
                Some(DirectTileLayoutGroundGeometry {
                    width,
                    height,
                    x_offs,
                    y_offs,
                }),
                "geometría NFO del waypoint {sprite_id}"
            );
            layout.sequence[0].sprite = None;
            layout.sequence[0].base_sprite = Some(sprite_id);
            assert!(
                !tile_layout_is_road_stop_renderable(&layout),
                "el namespace de paradas no debe aceptar el poste {sprite_id}"
            );
            assert!(
                tile_layout_is_road_waypoint_renderable(&layout),
                "el namespace de waypoint debe aceptar el poste {sprite_id}"
            );
        }

        layout.sequence[0].sprite = None;
        layout.sequence[0].base_sprite = Some(6145);
        assert_eq!(direct_tile_layout_road_waypoint_geometry(6145), None);
        assert!(!tile_layout_is_road_waypoint_renderable(&layout));
        layout.sequence[0].base_sprite = Some(6143);
        layout.sequence[0].direct_palette = 1;
        assert!(!tile_layout_is_road_waypoint_renderable(&layout));
    }

    #[test]
    fn direct_base_build_uses_consistent_industry_overlay_geometry() {
        assert_eq!(
            direct_tile_layout_industry_geometry(2011),
            Some(DirectTileLayoutGroundGeometry {
                width: 36.0,
                height: 25.0,
                x_offs: -17.0,
                y_offs: -7.0,
            })
        );
        assert_eq!(
            direct_tile_layout_industry_geometry(2047),
            Some(DirectTileLayoutGroundGeometry {
                width: 43.0,
                height: 56.0,
                x_offs: -21.0,
                y_offs: -34.0,
            })
        );
        assert_eq!(direct_tile_layout_industry_geometry(2000), None);
    }

    #[test]
    fn direct_base_ground_uses_consistent_industry_ground_geometry() {
        assert_eq!(
            direct_tile_layout_industry_ground_geometry(2022),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 31.0,
                x_offs: -31.0,
                y_offs: 0.0,
            })
        );
        assert_eq!(
            direct_tile_layout_industry_ground_geometry(2269),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 46.0,
                x_offs: -31.0,
                y_offs: -15.0,
            })
        );
        assert_eq!(
            direct_tile_layout_industry_ground_geometry(4769),
            Some(DirectTileLayoutGroundGeometry {
                width: 32.0,
                height: 32.0,
                x_offs: 0.0,
                y_offs: -1.0,
            })
        );
        assert_eq!(direct_tile_layout_industry_ground_geometry(2000), None);
    }

    #[test]
    fn direct_base_ground_uses_consistent_house_ground_geometry() {
        assert_eq!(
            direct_tile_layout_house_ground_geometry(1424),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 37.0,
                x_offs: -31.0,
                y_offs: -6.0,
            })
        );
        assert_eq!(
            direct_tile_layout_house_ground_geometry(4471),
            Some(DirectTileLayoutGroundGeometry {
                width: 44.0,
                height: 23.0,
                x_offs: -21.0,
                y_offs: 4.0,
            })
        );
        assert_eq!(
            direct_tile_layout_house_ground_geometry(4458),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 36.0,
                x_offs: -30.0,
                y_offs: -5.0,
            })
        );
        assert_eq!(direct_tile_layout_house_ground_geometry(2000), None);
    }

    #[test]
    fn direct_base_build_uses_consistent_house_overlay_geometry() {
        assert_eq!(
            direct_tile_layout_house_geometry(1421),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 37.0,
                x_offs: -31.0,
                y_offs: -6.0,
            })
        );
        assert_eq!(
            direct_tile_layout_house_geometry(1430),
            Some(DirectTileLayoutGroundGeometry {
                width: 35.0,
                height: 20.0,
                x_offs: -18.0,
                y_offs: 2.0,
            })
        );
        assert_eq!(direct_tile_layout_house_geometry(1419), None);
    }

    #[test]
    fn direct_base_build_uses_object_namespace_geometry() {
        assert_eq!(
            direct_tile_layout_object_geometry(1420),
            Some(DirectTileLayoutGroundGeometry {
                width: 64.0,
                height: 31.0,
                x_offs: -31.0,
                y_offs: 0.0,
            })
        );
        assert_eq!(
            direct_tile_layout_object_geometry(2601),
            Some(DirectTileLayoutGroundGeometry {
                width: 54.0,
                height: 94.0,
                x_offs: -26.0,
                y_offs: -80.0,
            })
        );
        assert_eq!(
            direct_tile_layout_object_geometry(2602),
            Some(DirectTileLayoutGroundGeometry {
                width: 21.0,
                height: 64.0,
                x_offs: -9.0,
                y_offs: -52.0,
            })
        );
        assert_eq!(
            direct_tile_layout_object_geometry(2632),
            Some(DirectTileLayoutGroundGeometry {
                width: 60.0,
                height: 45.0,
                x_offs: -30.0,
                y_offs: -42.0,
            })
        );
        assert_eq!(
            direct_tile_layout_object_geometry(4790),
            Some(DirectTileLayoutGroundGeometry {
                width: 32.0,
                height: 48.0,
                x_offs: -16.0,
                y_offs: -40.0,
            })
        );
        assert_eq!(direct_tile_layout_object_geometry(2600), None);
    }

    #[test]
    fn direct_object_build_ids_are_renderable_without_palette_modifiers() {
        let direct_ground = ResolvedTileLayoutSprite {
            sprite: None,
            base_sprite: Some(3981),
            sprite_modifiers: 0,
            direct_palette: 0,
            origin: [0, 0, 0],
            extent: [0, 0, 0],
        };
        let mut layout = ResolvedTileLayout {
            ground: Some(direct_ground),
            sequence: vec![action1_sprite()],
            complete: true,
        };
        for sprite_id in [1420, 2601, 2602, 2632, 4790] {
            layout.sequence[0].sprite = None;
            layout.sequence[0].base_sprite = Some(sprite_id);
            assert!(
                tile_layout_is_renderable(&layout),
                "sprite de objeto vanilla {sprite_id} debe conservarse como BUILD"
            );
        }
        layout.sequence[0].base_sprite = Some(2601);
        layout.sequence[0].direct_palette = 1;
        assert!(!tile_layout_is_renderable(&layout));
    }

    #[test]
    fn direct_object_hq_layout_is_contextual_and_keeps_global_collisions_isolated() {
        let direct_ground = ResolvedTileLayoutSprite {
            sprite: None,
            base_sprite: Some(3981),
            sprite_modifiers: 0,
            direct_palette: 0,
            origin: [0, 0, 0],
            extent: [0, 0, 0],
        };
        let mut layout = ResolvedTileLayout {
            ground: Some(direct_ground),
            sequence: vec![action1_sprite()],
            complete: true,
        };

        for sprite_id in [2603, 2612, 2626, 2631] {
            layout.sequence[0].sprite = None;
            layout.sequence[0].base_sprite = Some(sprite_id);
            assert!(
                !tile_layout_is_renderable(&layout),
                "HQ {sprite_id} no debe entrar al resolver global"
            );
            assert!(
                tile_layout_is_object_renderable(&layout),
                "HQ {sprite_id} debe entrar al resolver contextual de objetos"
            );
        }

        layout.sequence[0].base_sprite = Some(2603);
        layout.ground.as_mut().expect("ground").base_sprite = Some(2603);
        assert!(tile_layout_is_object_renderable(&layout));

        layout.sequence[0].direct_palette = 1;
        assert!(!tile_layout_is_object_renderable(&layout));
    }

    #[test]
    fn direct_road_stop_layout_is_contextual_and_rejects_unsupported_modifiers() {
        let direct_ground = ResolvedTileLayoutSprite {
            sprite: None,
            base_sprite: Some(3981),
            sprite_modifiers: 0,
            direct_palette: 0,
            origin: [0, 0, 0],
            extent: [0, 0, 0],
        };
        let mut layout = ResolvedTileLayout {
            ground: Some(direct_ground),
            sequence: vec![action1_sprite()],
            complete: true,
        };

        for sprite_id in [2692, 2696, 2708, 2712, 5978, 5985] {
            layout.sequence[0].sprite = None;
            layout.sequence[0].base_sprite = Some(sprite_id);
            assert!(
                tile_layout_is_road_stop_renderable(&layout),
                "road-stop {sprite_id} debe entrar al resolver contextual"
            );
        }

        layout.sequence[0].base_sprite = Some(2724);
        assert!(!tile_layout_is_road_stop_renderable(&layout));
        layout.sequence[0].base_sprite = Some(2696);
        layout.sequence[0].direct_palette = 1;
        assert!(!tile_layout_is_road_stop_renderable(&layout));
    }

    #[test]
    fn direct_base_ground_with_palette_modifier_keeps_atomic_fallback() {
        let layout = ResolvedTileLayout {
            ground: Some(ResolvedTileLayoutSprite {
                sprite: None,
                base_sprite: Some(3981),
                sprite_modifiers:
                    openttdrs_core::newgrf_sprites::TILE_LAYOUT_SPRITE_MODIFIER_RECOLOUR,
                direct_palette: 0,
                origin: [0, 0, 0],
                extent: [0, 0, 0],
            }),
            sequence: vec![action1_sprite()],
            complete: true,
        };
        assert!(!tile_layout_is_renderable(&layout));
    }
}
