//! Comprueba existencia de assets y expone su procedencia/calidad al HUD.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

const OPENMSX_TRACK_COUNT: usize = 31;

/// Raíz del repositorio (mismo `file_path` que [`bevy::asset::AssetPlugin`]).
#[derive(Resource, Clone)]
pub(crate) struct ClientAssetRoot(pub PathBuf);

impl ClientAssetRoot {
    #[must_use]
    pub fn asset_file_exists(&self, relative: &str) -> bool {
        self.0.join(relative).is_file()
    }
}

/// Resumen inmutable del paquete de assets que el cliente encontró al arrancar.
///
/// El HUD consume este recurso para no declarar una calidad que sólo estaba
/// configurada: el modo gráfico se lee del paquete instalado y la calidad de
/// audio se obtiene de una cabecera WAV/OGG real.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ClientAssetStatus {
    graphics: GraphicsAssetStatus,
    sfx: SfxAssetStatus,
    music: AudioAssetStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GraphicsAssetStatus {
    set_name: String,
    quality: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AudioAssetStatus {
    source: String,
    quality: String,
    available: usize,
    expected: usize,
}

/// Los sonidos de mundo y los pings de interfaz son catálogos distintos.
///
/// El cliente carga el primero exclusivamente como `snd_00.wav` …
/// `snd_72.wav`; no debemos presentar unos WAV auxiliares como si fuesen el
/// baseset OpenSFX completo.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SfxAssetStatus {
    quality: String,
    world_available: usize,
    ui_available: usize,
}

impl ClientAssetStatus {
    #[must_use]
    pub(crate) fn probe(root: &Path) -> Self {
        Self {
            graphics: probe_graphics(root),
            sfx: probe_sfx(root),
            music: probe_music(root),
        }
    }

    /// Etiqueta corta, pero verificable, para el HUD.
    #[must_use]
    pub(crate) fn graphics_hud_label(&self) -> String {
        format!("{} · {}", self.graphics.set_name, self.graphics.quality)
    }

    /// Muestra el catálogo que realmente puede reproducir el mixer.
    #[must_use]
    pub(crate) fn sfx_hud_label(&self) -> String {
        let sfx = &self.sfx;
        match (sfx.world_available, sfx.ui_available) {
            (0, 0) => "sin SFX".into(),
            (0, ui_available) => format!(
                "SFX UI {} · {} archivos; mundo 0/{}",
                sfx.quality,
                ui_available,
                openttdrs_core::sound_id::SOUND_COUNT
            ),
            (world_available, ui_available) => {
                let partial = if world_available < openttdrs_core::sound_id::SOUND_COUNT {
                    " parcial"
                } else {
                    ""
                };
                let ui = if ui_available > 0 {
                    format!(" · UI {ui_available}")
                } else {
                    String::new()
                };
                format!(
                    "OpenSFX {} {}/{}{}{}",
                    sfx.quality,
                    world_available,
                    openttdrs_core::sound_id::SOUND_COUNT,
                    partial,
                    ui
                )
            }
        }
    }

    /// Muestra el catálogo OGG disponible para la jukebox.
    #[must_use]
    pub(crate) fn music_hud_label(&self) -> String {
        audio_hud_label(&self.music)
    }
}

fn audio_hud_label(status: &AudioAssetStatus) -> String {
    if status.available == 0 {
        return status.source.clone();
    }
    let completeness = if status.available < status.expected {
        " parcial"
    } else {
        ""
    };
    format!(
        "{} {} {}/{}{}",
        status.source, status.quality, status.available, status.expected, completeness
    )
}

fn graphics_mode(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("assets/opengfx/.graphics_mode")).ok()?;
    let mode = text.trim();
    matches!(mode, "8bpp" | "32bpp").then(|| mode.to_owned())
}

fn first_dir_name_with_prefix(root: &Path, prefix: &str) -> Option<String> {
    let mut names: Vec<String> = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_type()
                .ok()
                .filter(std::fs::FileType::is_dir)
                .map(|_| entry)
        })
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(prefix))
        .collect();
    names.sort();
    names.into_iter().next()
}

fn probe_graphics(root: &Path) -> GraphicsAssetStatus {
    let opengfx = root.join("assets/opengfx");
    let mode = graphics_mode(root).or_else(|| {
        if opengfx.join("opengfx2-32ez").is_dir() {
            Some("32bpp".into())
        } else if first_dir_name_with_prefix(&opengfx, "opengfx-").is_some() {
            Some("8bpp".into())
        } else {
            None
        }
    });

    match mode.as_deref() {
        Some("8bpp") => {
            let name = first_dir_name_with_prefix(&opengfx, "opengfx-")
                .map(|name| name.replacen("opengfx-", "OpenGFX ", 1))
                .unwrap_or_else(|| "OpenGFX".into());
            GraphicsAssetStatus {
                set_name: name,
                quality: "8bpp".into(),
            }
        }
        Some("32bpp") => GraphicsAssetStatus {
            set_name: "OpenGFX2".into(),
            quality: "32bpp".into(),
        },
        _ => GraphicsAssetStatus {
            set_name: "sin gráficos base".into(),
            quality: "desconocida".into(),
        },
    }
}

fn files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        })
        .collect();
    paths.sort();
    paths
}

fn format_rate_hz(rate: u32) -> String {
    if rate.is_multiple_of(1_000) {
        format!("{} kHz", rate / 1_000)
    } else {
        format!("{:.1} kHz", rate as f32 / 1_000.0)
    }
}

fn channel_label(channels: u16) -> &'static str {
    match channels {
        1 => "mono",
        2 => "estéreo",
        _ => "multicanal",
    }
}

fn quality_summary(
    paths: &[PathBuf],
    family: &str,
    probe: impl Fn(&Path) -> Option<String>,
) -> String {
    let qualities: BTreeSet<String> = paths.iter().filter_map(|path| probe(path)).collect();
    match qualities.len() {
        0 => format!("{family} calidad desconocida"),
        1 => format!(
            "{family} {}",
            qualities.into_iter().next().unwrap_or_default()
        ),
        _ => {
            let samples = qualities.into_iter().take(2).collect::<Vec<_>>().join(", ");
            format!("{family} mixto ({samples})")
        }
    }
}

/// Lee el chunk `fmt ` de WAV PCM sin depender de un decodificador multimedia.
fn wav_quality(path: &Path) -> Option<String> {
    let data = fs::read(path).ok()?;
    if data.get(0..4)? != b"RIFF" || data.get(8..12)? != b"WAVE" {
        return None;
    }
    let mut offset = 12usize;
    while offset.checked_add(8)? <= data.len() {
        let id = data.get(offset..offset + 4)?;
        let length =
            u32::from_le_bytes(data.get(offset + 4..offset + 8)?.try_into().ok()?) as usize;
        let body = offset.checked_add(8)?;
        let end = body.checked_add(length)?;
        if end > data.len() {
            return None;
        }
        if id == b"fmt " && length >= 16 {
            let channels = u16::from_le_bytes(data.get(body + 2..body + 4)?.try_into().ok()?);
            let rate = u32::from_le_bytes(data.get(body + 4..body + 8)?.try_into().ok()?);
            let bits = u16::from_le_bytes(data.get(body + 14..body + 16)?.try_into().ok()?);
            return Some(format!(
                "{} {}b {}",
                format_rate_hz(rate),
                bits,
                channel_label(channels)
            ));
        }
        offset = end.checked_add(length % 2)?;
    }
    None
}

/// La cabecera de identificación Vorbis contiene canales y sample rate.
fn ogg_vorbis_quality(path: &Path) -> Option<String> {
    let data = fs::read(path).ok()?;
    let marker = b"\x01vorbis";
    let start = data
        .windows(marker.len())
        .position(|window| window == marker)?;
    let channels = u16::from(*data.get(start + marker.len() + 4)?);
    let rate_offset = start + marker.len() + 5;
    let rate = u32::from_le_bytes(data.get(rate_offset..rate_offset + 4)?.try_into().ok()?);
    Some(format!(
        "{} {}",
        format_rate_hz(rate),
        channel_label(channels)
    ))
}

fn probe_sfx(root: &Path) -> SfxAssetStatus {
    let dir = root.join("assets/sounds");
    let wavs = files_with_extension(&dir, "wav");
    let world_paths: Vec<PathBuf> = (0..openttdrs_core::sound_id::SOUND_COUNT)
        .map(|index| dir.join(format!("snd_{index:02}.wav")))
        .filter(|path| path.is_file())
        .collect();
    let quality_paths = if world_paths.is_empty() {
        &wavs
    } else {
        &world_paths
    };
    SfxAssetStatus {
        quality: quality_summary(quality_paths, "WAV", wav_quality),
        world_available: world_paths.len(),
        ui_available: wavs.len().saturating_sub(world_paths.len()),
    }
}

fn probe_music(root: &Path) -> AudioAssetStatus {
    let oggs = files_with_extension(&root.join("assets/music"), "ogg");
    AudioAssetStatus {
        source: if oggs.is_empty() {
            "sin OpenMSX".into()
        } else {
            "OpenMSX".into()
        },
        quality: quality_summary(&oggs, "OGG/Vorbis", ogg_vorbis_quality),
        available: oggs.len(),
        expected: OPENMSX_TRACK_COUNT,
    }
}

/// Registra la raíz de assets para sistemas de audio.
pub(crate) fn insert_asset_root(app: &mut App, asset_root: &str) {
    let root = PathBuf::from(asset_root);
    app.insert_resource(ClientAssetStatus::probe(&root));
    app.insert_resource(ClientAssetRoot(root));
}

pub(crate) fn warn_missing_optional_assets(root: &Path) {
    let sounds = root.join("assets/sounds");
    let music = root.join("assets/music");
    let need_sfx = !sounds.join("construction_water.wav").is_file();
    let need_music = !music.join("theme.ogg").is_file();
    if !need_sfx && !need_music {
        return;
    }
    eprintln!(
        "Aviso: faltan assets de audio (el repo los incluye tras clone; ¿working tree incompleto?)."
    );
    if need_sfx {
        eprintln!("  Sonidos: ./scripts/preparar_sonidos_opensfx.sh");
    }
    if need_music {
        eprintln!("  Música: ./scripts/preparar_musica_ogg.sh");
        eprintln!(
            "  (requiere OpenMSX + fluidsynth + ffmpeg; ver README § Dependencias opcionales)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wav(path: &Path, channels: u16, rate: u32, bits: u16) -> std::io::Result<()> {
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&36u32.to_le_bytes());
        data.extend_from_slice(b"WAVEfmt ");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&channels.to_le_bytes());
        data.extend_from_slice(&rate.to_le_bytes());
        data.extend_from_slice(&(rate * u32::from(channels) * u32::from(bits) / 8).to_le_bytes());
        data.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        data.extend_from_slice(&bits.to_le_bytes());
        data.extend_from_slice(b"data");
        data.extend_from_slice(&0u32.to_le_bytes());
        fs::write(path, data)
    }

    fn write_vorbis_header(path: &Path, channels: u8, rate: u32) -> std::io::Result<()> {
        let mut data = b"OggS\0\0".to_vec();
        data.extend_from_slice(b"\x01vorbis");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.push(channels);
        data.extend_from_slice(&rate.to_le_bytes());
        fs::write(path, data)
    }

    #[test]
    fn reports_installed_sets_and_audio_quality() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let root = dir.path();
        let opengfx = root.join("assets/opengfx");
        let sounds = root.join("assets/sounds");
        let music = root.join("assets/music");
        fs::create_dir_all(opengfx.join("opengfx-8.0"))?;
        fs::create_dir_all(&sounds)?;
        fs::create_dir_all(&music)?;
        fs::write(opengfx.join(".graphics_mode"), "8bpp\n")?;
        write_wav(&sounds.join("snd_00.wav"), 1, 44_100, 16)?;
        write_vorbis_header(&music.join("theme.ogg"), 2, 44_100)?;

        let status = ClientAssetStatus::probe(root);
        assert_eq!(status.graphics_hud_label(), "OpenGFX 8.0 · 8bpp");
        assert!(
            status
                .sfx_hud_label()
                .contains("OpenSFX WAV 44.1 kHz 16b mono 1/73 parcial")
        );
        assert!(
            status
                .music_hud_label()
                .contains("OGG/Vorbis 44.1 kHz estéreo 1/31 parcial")
        );
        Ok(())
    }

    #[test]
    fn ui_wavs_are_not_reported_as_a_complete_opensfx_catalog()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let sounds = dir.path().join("assets/sounds");
        fs::create_dir_all(&sounds)?;
        write_wav(&sounds.join("hud_soft.wav"), 1, 22_050, 16)?;
        write_wav(&sounds.join("construction_water.wav"), 1, 44_100, 16)?;

        let status = ClientAssetStatus::probe(dir.path());
        let label = status.sfx_hud_label();
        assert!(label.starts_with("SFX UI WAV mixto"));
        assert!(label.contains("2 archivos; mundo 0/73"));
        assert!(!label.contains("OpenSFX"));
        Ok(())
    }

    #[test]
    fn unavailable_sets_do_not_claim_a_quality() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let status = ClientAssetStatus::probe(dir.path());
        assert_eq!(
            status.graphics_hud_label(),
            "sin gráficos base · desconocida"
        );
        assert_eq!(status.sfx_hud_label(), "sin SFX");
        assert_eq!(status.music_hud_label(), "sin OpenMSX");
        Ok(())
    }
}
