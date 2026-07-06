//! Comprueba existencia de archivos de audio antes de cargarlos en Bevy.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

/// Raíz del repositorio (mismo `file_path` que [`bevy::asset::AssetPlugin`]).
#[derive(Resource, Clone)]
pub(crate) struct ClientAssetRoot(pub PathBuf);

impl ClientAssetRoot {
    #[must_use]
    pub fn asset_file_exists(&self, relative: &str) -> bool {
        self.0.join(relative).is_file()
    }
}

/// Registra la raíz de assets para sistemas de audio.
pub(crate) fn insert_asset_root(app: &mut App, asset_root: &str) {
    app.insert_resource(ClientAssetRoot(PathBuf::from(asset_root)));
}

pub(crate) fn warn_missing_optional_assets(root: &Path) {
    let sounds = root.join("assets/sounds");
    let music = root.join("assets/music");
    let need_sfx = !sounds.join("construction_water.wav").is_file();
    let need_music = !music.join("theme.ogg").is_file();
    if !need_sfx && !need_music {
        return;
    }
    eprintln!("Aviso: faltan assets de audio opcionales.");
    if need_sfx {
        eprintln!("  Sonidos: ./scripts/preparar_sonidos_opensfx.sh");
        eprintln!("  (o ./scripts/descargar_assets.sh sonidos)");
    }
    if need_music {
        eprintln!("  Música: ./scripts/descargar_musica.sh --openmsx");
        eprintln!("  (requiere fluidsynth + SoundFont para generar assets/music/*.ogg)");
    }
}
