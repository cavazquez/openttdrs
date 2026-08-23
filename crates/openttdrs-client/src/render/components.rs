use std::collections::HashMap;

use bevy::prelude::*;

use crate::render::atlas::AtlasSprite;

/// Marca superficies acuáticas y distingue las que usan la paleta animada.
///
/// Esclusas y otras estructuras se limpian junto con el agua, pero conservan
/// su sprite estático en vez de ser reemplazadas por `water_anim_*`.
#[derive(Component, Clone, Copy)]
#[allow(dead_code)] // conserva la semantica animada/estatica para inspeccion y NewGRF
pub(crate) struct WaterTile {
    palette_animated: bool,
}

impl WaterTile {
    pub(crate) const ANIMATED: Self = Self {
        palette_animated: true,
    };
    pub(crate) const STATIC: Self = Self {
        palette_animated: false,
    };

    #[must_use]
    #[allow(dead_code)]
    pub(crate) const fn is_palette_animated(self) -> bool {
        self.palette_animated
    }
}

/// Tesela de orilla: slot `i` de `shore_full_{i:02}.png` para ciclar sus frames.
#[derive(Component)]
#[allow(dead_code)] // slot semantico para NewGRF/diagnostico; el atlas anima globalmente
pub(crate) struct ShoreTile(pub(crate) u8);

/// Entrada compartida del atlas que se redirige globalmente entre frames.
pub(crate) struct WaterAtlasAnimation {
    pub(crate) layout: Handle<TextureAtlasLayout>,
    pub(crate) target_index: usize,
    pub(crate) frame_rects: Vec<URect>,
}

/// 75 combinaciones pre-horneadas, indexadas como `dark * 15 + glitter`.
///
/// Se mutan 19 rects compartidos del atlas, no cada entidad de agua del mapa.
#[derive(Resource)]
pub(crate) struct WaterAnimFrames {
    pub(crate) water: Option<WaterAtlasAnimation>,
    pub(crate) shore: Vec<WaterAtlasAnimation>,
}

/// Frames del fuego de refinería (`industry_{id}_fire_anim_{f}.png`), 7 pasos.
#[derive(Resource)]
pub(crate) struct RefineryFireAnimFrames {
    pub(crate) by_sprite: HashMap<u32, Vec<AtlasSprite>>,
}

/// Frames bebidas gaseosas (`industry_{id}_fizzy_anim_{f}.png`), 5 pasos.
#[derive(Resource)]
pub(crate) struct FizzyDrinkAnimFrames {
    pub(crate) by_sprite: HashMap<u32, Vec<AtlasSprite>>,
}

/// Frames faro/estadio (`object_lighthouse_anim_*` / `house_s*_anim_*`), 4 pasos.
#[derive(Resource)]
pub(crate) struct LighthouseAnimFrames {
    pub(crate) by_sprite: HashMap<u32, Vec<AtlasSprite>>,
}

/// Teselas de suelo, vías, vehículos, etc.: se despawnan al recargar JSON (F9).
#[derive(Component)]
pub(crate) struct MapVisualLayer;

/// Metadatos comunes a los dos nodos de cada cartel (fondo y texto).
///
/// OpenTTD mantiene los carteles legibles en zooms alejados. Guardar el tamaño
/// base de ambas variantes permite cambiar a `FS_SMALL` sin depender del orden
/// ECS de cada frame.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct MapLabelLod {
    /// Caja del texto normal (Out2x e inferiores).
    pub(crate) size: Vec2,
    /// Caja de la variante pequeña (Out4x/Out8x), como en `ViewportSign`.
    pub(crate) small_size: Vec2,
}

/// Texto normal y reducido de un cartel. OpenTTD cambia a la cadena pequeña
/// desde `Out4x`; mantener ambas evita que el LOD sólo escale visualmente un
/// texto largo y termine ocultando casi todos los nombres por colisión.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub(crate) struct MapLabelText {
    pub(crate) normal: String,
    pub(crate) small: String,
}

/// Bloque espacial del mapa (16×16 teselas) para culling incremental al hacer pan.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MapTileChunk {
    pub cx: u32,
    pub cy: u32,
}

pub(crate) const MAP_TILE_CHUNK_SIZE: u32 = 16;

impl MapTileChunk {
    #[must_use]
    pub fn from_tile(tx: u32, ty: u32) -> Self {
        Self {
            cx: tx / MAP_TILE_CHUNK_SIZE,
            cy: ty / MAP_TILE_CHUNK_SIZE,
        }
    }
}

/// Cámara isométrica principal (ventana). Distingue de [`MapPreviewCamera`].
#[derive(Component)]
pub(crate) struct PrimaryGameCamera;

/// Cámaras que renderizan el mapa a una textura (no la ventana principal).
#[derive(Component)]
pub(crate) struct MapPreviewCamera;

/// Vista previa de industria en el panel lateral.
#[derive(Component)]
pub(crate) struct IndustryPreviewCamera;

#[derive(Default)]
pub(crate) struct MapSpriteBatches {
    pub(super) water: Vec<(MapTileChunk, WaterTile, Sprite, Transform)>,
    pub(super) shore: Vec<(MapTileChunk, ShoreTile, Sprite, Transform)>,
}
