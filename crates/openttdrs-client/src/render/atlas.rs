//! Texture atlas de las teselas del mapa.
//!
//! Todos los sprites de `assets/opengfx/tiles/` viven en páginas únicas
//! (`assets/opengfx/atlas/tiles_atlas_{p}.png`, generadas por
//! `scripts/gen_tile_atlas.py`). Compartir textura permite que el renderer 2D
//! de Bevy agrupe los miles de sprites del mapa en pocos draw calls; con PNGs
//! sueltos cada cambio de textura cortaba el batch.

use bevy::prelude::*;

use crate::sprites::{
    TILE_ATLAS_NAMES, TILE_ATLAS_PAGE_COUNT, TILE_ATLAS_PAGE_RANGES, TILE_ATLAS_PAGE_SIZES,
    TILE_ATLAS_RECTS,
};

/// Sprite resuelto dentro del atlas: textura de página + rect.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AtlasSprite {
    pub(crate) image: Handle<Image>,
    pub(crate) atlas: TextureAtlas,
}

impl AtlasSprite {
    pub(crate) fn sprite(&self) -> Sprite {
        self.sprite_colored(Color::WHITE)
    }

    pub(crate) fn sprite_colored(&self, color: Color) -> Sprite {
        Sprite {
            image: self.image.clone(),
            texture_atlas: Some(self.atlas.clone()),
            color,
            ..default()
        }
    }

    /// Aplica este sprite del atlas sobre un `Sprite` existente (animaciones).
    pub(crate) fn apply_to(&self, sprite: &mut Sprite) {
        sprite.image = self.image.clone();
        sprite.texture_atlas = Some(self.atlas.clone());
    }

    /// ¿`sprite` ya muestra esta entrada del atlas?
    pub(crate) fn matches(&self, sprite: &Sprite) -> bool {
        sprite.texture_atlas.as_ref() == Some(&self.atlas) && sprite.image == self.image
    }
}

/// Páginas y layouts del atlas de teselas (recurso global, se crea una vez).
#[derive(Resource)]
pub(crate) struct TileAtlas {
    pages: Vec<Handle<Image>>,
    layouts: Vec<Handle<TextureAtlasLayout>>,
}

impl TileAtlas {
    pub(crate) fn build(
        asset_server: &AssetServer,
        layout_assets: &mut Assets<TextureAtlasLayout>,
    ) -> Self {
        let pages = (0..TILE_ATLAS_PAGE_COUNT)
            .map(|p| {
                asset_server.load::<Image>(format!("assets/opengfx/atlas/tiles_atlas_{p}.png"))
            })
            .collect();
        let layouts = (0..TILE_ATLAS_PAGE_COUNT)
            .map(|p| {
                let (pw, ph) = TILE_ATLAS_PAGE_SIZES[p];
                let mut layout = TextureAtlasLayout::new_empty(UVec2::new(pw, ph));
                let (start, end) = TILE_ATLAS_PAGE_RANGES[p];
                for &(_page, x, y, w, h) in &TILE_ATLAS_RECTS[start as usize..end as usize] {
                    layout.add_texture(URect::new(
                        u32::from(x),
                        u32::from(y),
                        u32::from(x) + u32::from(w),
                        u32::from(y) + u32::from(h),
                    ));
                }
                layout_assets.add(layout)
            })
            .collect();
        Self { pages, layouts }
    }

    /// Busca `name` (p. ej. `"grass.png"`) en la tabla generada.
    pub(crate) fn try_get(&self, name: &str) -> Option<AtlasSprite> {
        let i = TILE_ATLAS_NAMES
            .binary_search_by(|(n, _)| (*n).cmp(name))
            .ok()?;
        let rect_idx = TILE_ATLAS_NAMES[i].1;
        let page = usize::from(TILE_ATLAS_RECTS[rect_idx as usize].0);
        let (start, _end) = TILE_ATLAS_PAGE_RANGES[page];
        Some(AtlasSprite {
            image: self.pages[page].clone(),
            atlas: TextureAtlas {
                layout: self.layouts[page].clone(),
                index: (rect_idx - start) as usize,
            },
        })
    }

    /// Como `try_get`, pero ante un nombre desconocido loguea el error y
    /// devuelve la primera entrada del atlas (equivalente al «sprite rosa»
    /// de textura faltante: visible pero no fatal).
    pub(crate) fn get(&self, name: &str) -> AtlasSprite {
        self.try_get(name).unwrap_or_else(|| {
            error!("Sprite no encontrado en el atlas de teselas: {name}");
            AtlasSprite {
                image: self.pages[0].clone(),
                atlas: TextureAtlas {
                    layout: self.layouts[0].clone(),
                    index: 0,
                },
            }
        })
    }

    /// Acepta tanto `"grass.png"` como `"assets/opengfx/tiles/grass.png"`.
    pub(crate) fn get_path(&self, path: &str) -> AtlasSprite {
        let name = path.rsplit('/').next().unwrap_or(path);
        self.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_atlas() -> (TileAtlas, Assets<TextureAtlasLayout>) {
        let mut layouts = Assets::<TextureAtlasLayout>::default();
        let pages = (0..TILE_ATLAS_PAGE_COUNT)
            .map(|p| {
                Handle::Uuid(
                    bevy::asset::uuid::Uuid::from_u128(p as u128 + 1),
                    std::marker::PhantomData,
                )
            })
            .collect();
        let layout_handles = (0..TILE_ATLAS_PAGE_COUNT)
            .map(|p| {
                let (pw, ph) = TILE_ATLAS_PAGE_SIZES[p];
                let mut layout = TextureAtlasLayout::new_empty(UVec2::new(pw, ph));
                let (start, end) = TILE_ATLAS_PAGE_RANGES[p];
                for &(_page, x, y, w, h) in &TILE_ATLAS_RECTS[start as usize..end as usize] {
                    layout.add_texture(URect::new(
                        u32::from(x),
                        u32::from(y),
                        u32::from(x) + u32::from(w),
                        u32::from(y) + u32::from(h),
                    ));
                }
                layouts.add(layout)
            })
            .collect();
        (
            TileAtlas {
                pages,
                layouts: layout_handles,
            },
            layouts,
        )
    }

    #[test]
    fn names_table_is_sorted_for_binary_search() {
        assert!(TILE_ATLAS_NAMES.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn rects_fit_inside_their_page() {
        for &(page, x, y, w, h) in TILE_ATLAS_RECTS {
            let (pw, ph) = TILE_ATLAS_PAGE_SIZES[usize::from(page)];
            assert!(u32::from(x) + u32::from(w) <= pw);
            assert!(u32::from(y) + u32::from(h) <= ph);
        }
    }

    #[test]
    fn refinery_fire_anim_frames_resolve() {
        let (atlas, _layouts) = test_atlas();
        for &id in &crate::sprites::REFINERY_FIRE_SPRITE_IDS {
            let frames: Vec<_> = (0..7)
                .filter_map(|f| atlas.try_get(&format!("industry_{id}_fire_anim_{f:02}.png")))
                .collect();
            assert_eq!(frames.len(), 7, "sprite {id} fire frames");
        }
    }

    #[test]
    fn known_tiles_resolve() {
        let (atlas, _layouts) = test_atlas();
        for name in ["grass.png", "water.png", "rail_1011.png"] {
            assert!(atlas.try_get(name).is_some(), "falta {name}");
        }
        assert!(atlas.try_get("no_existe.png").is_none());
    }

    #[test]
    fn aliases_share_the_same_rect() {
        // rail_1011/1012 son archivos reales distintos; los aliases generados
        // por descargar_graficos.sh (mismo contenido) deben compartir rect.
        let (atlas, _layouts) = test_atlas();
        let a = atlas.get("grass.png");
        let b = atlas.get_path("assets/opengfx/tiles/grass.png");
        assert_eq!(a, b);
    }

    #[test]
    fn water_global_animation_covers_flat_water_and_all_shores() {
        let (atlas, layouts) = test_atlas();
        let mut images = Assets::<Image>::default();
        let assets = crate::render::WorldAssets::load(&atlas, &mut images);
        let frames = crate::render::water_anim_frames_from_assets(&assets, &layouts);
        assert!(frames.water.is_some());
        assert_eq!(frames.shore.len(), crate::sprites::SHORE_SPRITE_COUNT);
        assert_eq!(
            frames.water.as_ref().map(|anim| anim.frame_rects.len()),
            Some(crate::sprites::WATER_PALETTE_FRAME_COUNT)
        );
        assert!(
            frames.shore.iter().all(|shore| {
                shore.frame_rects.len() == crate::sprites::WATER_PALETTE_FRAME_COUNT
            })
        );
    }

    #[test]
    fn get_unknown_falls_back_to_first_entry() {
        let (atlas, _layouts) = test_atlas();
        let fallback = atlas.get("no_existe.png");
        assert_eq!(fallback.atlas.index, 0);
    }

    #[test]
    fn apply_and_matches_roundtrip() {
        let (atlas, _layouts) = test_atlas();
        let entry = atlas.get("water.png");
        let mut sprite = Sprite::default();
        assert!(!entry.matches(&sprite));
        entry.apply_to(&mut sprite);
        assert!(entry.matches(&sprite));
    }
}
