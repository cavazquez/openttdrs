//! Contrato JSONL de bytes crudos por tesela para depurar paridad de `.sav`.
//!
//! El objetivo es comparar el mapa vivo de `OpenTTD` con las dos etapas locales
//! (`sav_map` y `game_state_map`) sin resumir los bytes que deciden un sprite.
//! Las filas se emiten en orden fila-mayor (`index = y * width + x`) y no se
//! acumulan en memoria.

use crate::map::{Map, Tile};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::io::{self, Write};

/// Versión actual del contrato `world-raw`.
pub const WORLD_RAW_SCHEMA_VERSION: u32 = 2;
/// Nombre estable del contrato, incluido en la primera línea JSONL.
pub const WORLD_RAW_CONTRACT: &str = "world-raw";

/// Región rectangular inclusiva de teselas, expresada en coordenadas absolutas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WorldRawRegion {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
}

impl WorldRawRegion {
    /// Construye una región inclusiva, rechazando límites invertidos.
    #[must_use]
    pub const fn new(min_x: u32, min_y: u32, max_x: u32, max_y: u32) -> Option<Self> {
        if min_x > max_x || min_y > max_y {
            return None;
        }
        Some(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }
}

/// Primera fila del stream JSONL `world-raw`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorldRawMetadata {
    /// Siempre `"metadata"`; permite mezclar la cabecera y las teselas en JSONL.
    #[serde(rename = "kind")]
    pub record_kind: &'static str,
    pub schema_version: u32,
    pub contract: &'static str,
    /// `openttd` para el oráculo C++ u `openttdrs` para este dumper.
    pub producer: String,
    /// Punto de observación, p. ej. `after_load_game` o `sav_map`.
    pub stage: String,
    /// `TimerGameTick::counter` del save cuando estaba disponible.
    pub tick: Option<u64>,
    /// `LandscapeType` de `OpenTTD` (0=temperate, 1=arctic, 2=tropic, 3=toyland).
    pub climate: Option<u8>,
    pub openttd_commit: String,
    pub source_path: String,
    pub save_sha256: String,
    pub save_version: Option<u16>,
    pub width: u32,
    pub height: u32,
    /// Teselas del mapa completo, independientemente de `region`.
    pub tile_count: u64,
    /// Filas `tile_raw` esperadas después de aplicar `region`.
    pub emitted_tile_count: u64,
    /// Filtro solicitado; `null` significa el mapa completo.
    pub region: Option<WorldRawRegion>,
}

/// Contexto de un dump `world-raw` que no se obtiene del mapa en sí.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldRawContext {
    pub producer: String,
    pub stage: String,
    pub tick: Option<u64>,
    pub climate: Option<u8>,
    pub openttd_commit: String,
    pub source_path: String,
    pub save_sha256: String,
    pub save_version: Option<u16>,
    pub region: Option<WorldRawRegion>,
}

impl WorldRawMetadata {
    /// Crea la cabecera a partir de un mapa y del contexto de carga conocido.
    #[must_use]
    pub fn for_map(map: &Map, context: &WorldRawContext) -> Self {
        let (width, height) = map.dimensions();
        Self {
            record_kind: "metadata",
            schema_version: WORLD_RAW_SCHEMA_VERSION,
            contract: WORLD_RAW_CONTRACT,
            producer: context.producer.clone(),
            stage: context.stage.clone(),
            tick: context.tick,
            climate: context.climate,
            openttd_commit: context.openttd_commit.clone(),
            source_path: context.source_path.clone(),
            save_sha256: context.save_sha256.clone(),
            save_version: context.save_version,
            width,
            height,
            tile_count: u64::from(width) * u64::from(height),
            emitted_tile_count: emitted_tile_count(width, height, context.region),
            region: context.region,
        }
    }
}

/// Una fila cruda de tesela. `type` es el byte MAPT completo de `OpenTTD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WorldRawTile {
    #[serde(rename = "kind")]
    pub record_kind: &'static str,
    pub index: u64,
    pub x: u32,
    pub y: u32,
    pub height: u8,
    #[serde(rename = "type")]
    pub tile_type: u8,
    pub m1: u8,
    pub m2: u16,
    pub m3: u8,
    /// Nombre de `OpenTTD` para el byte que `.ottdmap` almacena como `m3hi`.
    pub m4: u8,
    pub m5: u8,
    pub m6: u8,
    pub m7: u8,
    pub m8: u16,
}

impl WorldRawTile {
    #[must_use]
    fn from_map_tile(index: u64, x: u32, y: u32, tile: Tile) -> Self {
        Self {
            record_kind: "tile_raw",
            index,
            x,
            y,
            height: tile.height,
            tile_type: tile.mapt,
            m1: tile.m1,
            m2: u16::from(tile.m2) | (u16::from(tile.m2_hi) << 8),
            m3: tile.m3,
            m4: tile.m3hi,
            m5: tile.m5,
            m6: tile.m6,
            m7: tile.m7,
            m8: tile.m8,
        }
    }
}

/// Resultado compacto de una escritura de stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldRawDumpSummary {
    pub emitted_tile_count: u64,
}

/// SHA-256 hexadecimal minúsculo de los bytes exactos del `.sav`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// Escribe un stream JSONL de metadatos y filas crudas en orden fila-mayor.
///
/// La región, cuando existe, conserva las coordenadas e índices absolutos del
/// mapa. Así puede usarse un dump pequeño para señalar una divergencia sin
/// cambiar la identidad de la tesela.
///
/// # Errors
///
/// Devuelve el error de escritura o de serialización del destino.
pub fn write_world_raw_jsonl<W: Write>(
    writer: &mut W,
    metadata: &WorldRawMetadata,
    map: &Map,
) -> io::Result<WorldRawDumpSummary> {
    let (width, height) = map.dimensions();
    if metadata.width != width || metadata.height != height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "world-raw metadata no coincide con las dimensiones del mapa",
        ));
    }

    write_json_line(writer, metadata)?;
    let Some((min_x, min_y, max_x, max_y)) = effective_bounds(width, height, metadata.region)
    else {
        return Ok(WorldRawDumpSummary {
            emitted_tile_count: 0,
        });
    };

    let mut emitted = 0_u64;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let index = u64::from(y) * u64::from(width) + u64::from(x);
            let x_i32 = i32::try_from(x).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "world-raw encontró x fuera del rango i32",
                )
            })?;
            let y_i32 = i32::try_from(y).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "world-raw encontró y fuera del rango i32",
                )
            })?;
            let coord = crate::map::TileCoord::new(x_i32, y_i32);
            let tile = map.get(coord).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "world-raw encontró una tesela fuera de mapa",
                )
            })?;
            write_json_line(writer, &WorldRawTile::from_map_tile(index, x, y, tile))?;
            emitted += 1;
        }
    }
    if emitted != metadata.emitted_tile_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "world-raw emitió una cantidad distinta a la declarada en metadata",
        ));
    }
    Ok(WorldRawDumpSummary {
        emitted_tile_count: emitted,
    })
}

fn write_json_line<W: Write, T: Serialize>(writer: &mut W, value: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|error| io::Error::other(error.to_string()))?;
    writer.write_all(b"\n")
}

fn effective_bounds(
    width: u32,
    height: u32,
    region: Option<WorldRawRegion>,
) -> Option<(u32, u32, u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let full = WorldRawRegion {
        min_x: 0,
        min_y: 0,
        max_x: width - 1,
        max_y: height - 1,
    };
    let requested = region.unwrap_or(full);
    let min_x = requested.min_x.max(full.min_x);
    let min_y = requested.min_y.max(full.min_y);
    let max_x = requested.max_x.min(full.max_x);
    let max_y = requested.max_y.min(full.max_y);
    if min_x > max_x || min_y > max_y {
        return None;
    }
    Some((min_x, min_y, max_x, max_y))
}

fn emitted_tile_count(width: u32, height: u32, region: Option<WorldRawRegion>) -> u64 {
    let Some((min_x, min_y, max_x, max_y)) = effective_bounds(width, height, region) else {
        return 0;
    };
    u64::from(max_x - min_x + 1) * u64::from(max_y - min_y + 1)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{
        WorldRawContext, WorldRawMetadata, WorldRawRegion, sha256_hex, write_world_raw_jsonl,
    };
    use crate::map::{Map, TileCoord};

    fn metadata(map: &Map, region: Option<WorldRawRegion>) -> WorldRawMetadata {
        let context = WorldRawContext {
            producer: "openttdrs".to_string(),
            stage: "sav_map".to_string(),
            tick: Some(12_345),
            climate: Some(0),
            openttd_commit: "commit".to_string(),
            source_path: "/tmp/test.sav".to_string(),
            save_sha256: "a".repeat(64),
            save_version: Some(300),
            region,
        };
        WorldRawMetadata::for_map(map, &context)
    }

    #[test]
    fn emits_full_raw_tile_in_row_major_order() {
        let mut map = Map::new_flat(2, 2, 1);
        let mut tile = map.get(TileCoord::new(1, 0)).expect("tile");
        tile.height = 9;
        tile.mapt = 0x91;
        tile.m1 = 0x12;
        tile.m2 = 0x34;
        tile.m2_hi = 0x56;
        tile.m3 = 0x78;
        tile.m3hi = 0x9A;
        tile.m5 = 0xBC;
        tile.m6 = 0xDE;
        tile.m7 = 0xF0;
        tile.m8 = 0x1357;
        map.set_tile(TileCoord::new(1, 0), tile).expect("set tile");

        let mut out = Vec::new();
        let result = write_world_raw_jsonl(&mut out, &metadata(&map, None), &map).expect("dump");
        assert_eq!(result.emitted_tile_count, 4);

        let rows: Vec<serde_json::Value> = std::str::from_utf8(&out)
            .expect("utf8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0]["kind"], "metadata");
        assert_eq!(rows[1]["index"], 0);
        assert_eq!(rows[2]["index"], 1);
        assert_eq!(rows[2]["x"], 1);
        assert_eq!(rows[2]["y"], 0);
        assert_eq!(rows[2]["type"], 0x91);
        assert_eq!(rows[2]["m2"], 0x5634);
        assert_eq!(rows[2]["m4"], 0x9A);
        assert_eq!(rows[2]["m8"], 0x1357);
        assert_eq!(rows[4]["index"], 3);
        assert_eq!(rows[4]["x"], 1);
        assert_eq!(rows[4]["y"], 1);
    }

    #[test]
    fn region_preserves_absolute_coordinates_and_indices() {
        let map = Map::new_flat(4, 3, 0);
        let region = WorldRawRegion::new(2, 1, 9, 9).expect("region");
        let mut out = Vec::new();
        write_world_raw_jsonl(&mut out, &metadata(&map, Some(region)), &map).expect("dump");
        let rows: Vec<serde_json::Value> = std::str::from_utf8(&out)
            .expect("utf8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(rows[0]["emitted_tile_count"], 4);
        assert_eq!(rows[1]["index"], 6);
        assert_eq!(rows[1]["x"], 2);
        assert_eq!(rows[1]["y"], 1);
        assert_eq!(rows.last().expect("last")["index"], 11);
    }

    #[test]
    fn sha256_is_stable_and_standard() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn inverted_region_is_rejected() {
        assert!(WorldRawRegion::new(2, 1, 1, 2).is_none());
    }
}
