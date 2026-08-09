//! Exporta la interpretación semántica de cada tesela de un `.sav` (#306).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use openttdrs_core::sav;
use openttdrs_core::world_raw::{WorldRawRegion, sha256_hex};
use openttdrs_core::world_semantic::{
    WorldSemanticContext, WorldSemanticMetadata, write_world_semantic_jsonl,
};
use openttdrs_core::{Climate, GameState};

#[derive(Debug, Clone, Copy)]
enum Stage {
    SavMap,
    GameStateMap,
}

impl Stage {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "sav_map" | "sav-map" => Ok(Self::SavMap),
            "game_state_map" | "game-state-map" => Ok(Self::GameStateMap),
            _ => Err(format!(
                "--stage inválido: {value} (usar sav_map o game_state_map)"
            )),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::SavMap => "sav_map",
            Self::GameStateMap => "game_state_map",
        }
    }
}

struct Args {
    save: PathBuf,
    out: PathBuf,
    stage: Stage,
    region: Option<WorldRawRegion>,
    openttd_commit: String,
}

fn print_usage() {
    eprintln!(
        "uso: world_semantic_dumper <partida.sav> <salida.jsonl> [opciones]\n\
         \n\
         Opciones:\n\
           --stage sav_map|game_state_map  etapa a exportar (default: sav_map)\n\
           --region x0,y0,x1,y1            rectángulo inclusivo de teselas\n\
           --tile x,y [--radius N]          tesela, opcionalmente con contexto\n\
           --openttd-commit SHA             manifiesto del oráculo para metadata\n\
         \n\
         Sin filtro exporta el mapa completo en orden y * width + x."
    );
}

fn next_value(args: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("falta el valor de {option}"))
}

fn parse_u32(value: &str, label: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|error| format!("{label} inválido ({value:?}): {error}"))
}

fn parse_pair(value: &str, label: &str) -> Result<(u32, u32), String> {
    let mut values = value.split(',');
    let x = values
        .next()
        .ok_or_else(|| format!("{label} debe tener dos coordenadas"))?;
    let y = values
        .next()
        .ok_or_else(|| format!("{label} debe tener dos coordenadas"))?;
    if values.next().is_some() {
        return Err(format!("{label} debe tener exactamente dos coordenadas"));
    }
    Ok((parse_u32(x, label)?, parse_u32(y, label)?))
}

fn parse_region(value: &str) -> Result<WorldRawRegion, String> {
    let mut values = value.split(',');
    let min_x = values
        .next()
        .ok_or("--region debe tener cuatro coordenadas")?;
    let min_y = values
        .next()
        .ok_or("--region debe tener cuatro coordenadas")?;
    let max_x = values
        .next()
        .ok_or("--region debe tener cuatro coordenadas")?;
    let max_y = values
        .next()
        .ok_or("--region debe tener cuatro coordenadas")?;
    if values.next().is_some() {
        return Err("--region debe tener exactamente cuatro coordenadas".to_string());
    }
    WorldRawRegion::new(
        parse_u32(min_x, "--region")?,
        parse_u32(min_y, "--region")?,
        parse_u32(max_x, "--region")?,
        parse_u32(max_y, "--region")?,
    )
    .ok_or_else(|| "--region tiene límites invertidos".to_string())
}

fn parse_args() -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut stage = Stage::SavMap;
    let mut region = None;
    let mut tile = None;
    let mut radius = None;
    let mut openttd_commit = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--stage" => stage = Stage::parse(&next_value(&mut args, "--stage")?)?,
            "--region" => {
                if region.is_some() {
                    return Err("--region fue indicado más de una vez".to_string());
                }
                region = Some(parse_region(&next_value(&mut args, "--region")?)?);
            }
            "--tile" => {
                if tile.is_some() {
                    return Err("--tile fue indicado más de una vez".to_string());
                }
                tile = Some(parse_pair(&next_value(&mut args, "--tile")?, "--tile")?);
            }
            "--radius" => {
                if radius.is_some() {
                    return Err("--radius fue indicado más de una vez".to_string());
                }
                radius = Some(parse_u32(&next_value(&mut args, "--radius")?, "--radius")?);
            }
            "--openttd-commit" => {
                openttd_commit = Some(next_value(&mut args, "--openttd-commit")?);
            }
            arg if arg.starts_with('-') => return Err(format!("opción desconocida: {arg}")),
            path => positional.push(PathBuf::from(path)),
        }
    }

    if positional.len() != 2 {
        return Err("se requieren <partida.sav> y <salida.jsonl>".to_string());
    }
    if region.is_some() && tile.is_some() {
        return Err("--region y --tile no pueden usarse juntos".to_string());
    }
    if radius.is_some() && tile.is_none() {
        return Err("--radius requiere --tile".to_string());
    }
    let region = match (region, tile, radius.unwrap_or(0)) {
        (Some(region), None, _) => Some(region),
        (None, Some((x, y)), radius) => WorldRawRegion::new(
            x.saturating_sub(radius),
            y.saturating_sub(radius),
            x.saturating_add(radius),
            y.saturating_add(radius),
        ),
        (None, None, _) => None,
        (Some(_), Some(_), _) => unreachable!("validado antes"),
    };
    Ok(Args {
        save: positional.remove(0),
        out: positional.remove(0),
        stage,
        region,
        openttd_commit: openttd_commit
            .or_else(|| std::env::var("OPENTTDRS_OPENTTD_COMMIT").ok())
            .unwrap_or_default(),
    })
}

fn canonical_source_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

const fn climate_code(climate: Climate) -> u8 {
    match climate {
        Climate::Temperate => 0,
        Climate::SubArctic => 1,
        Climate::SubTropical => 2,
        Climate::Toyland => 3,
    }
}

fn dump_map(
    map: &openttdrs_core::Map,
    args: &Args,
    source_path: String,
    save_sha256: String,
    save_version: u16,
    tick: Option<u64>,
    climate: u8,
) -> Result<u64, String> {
    let context = WorldSemanticContext {
        producer: "openttdrs".to_string(),
        stage: args.stage.as_str().to_string(),
        tick,
        climate: Some(climate),
        openttd_commit: args.openttd_commit.clone(),
        source_path,
        save_sha256,
        save_version: Some(save_version),
        region: args.region,
    };
    let metadata = WorldSemanticMetadata::for_map(map, &context);
    let file = File::create(&args.out)
        .map_err(|error| format!("no se pudo crear {}: {error}", args.out.display()))?;
    let mut writer = BufWriter::new(file);
    let summary = write_world_semantic_jsonl(&mut writer, &metadata, map)
        .map_err(|error| format!("no se pudo escribir {}: {error}", args.out.display()))?;
    writer
        .flush()
        .map_err(|error| format!("no se pudo cerrar {}: {error}", args.out.display()))?;
    Ok(summary.emitted_tile_count)
}

fn run(args: &Args) -> Result<(), String> {
    let raw = std::fs::read(&args.save)
        .map_err(|error| format!("no se pudo leer {}: {error}", args.save.display()))?;
    let save_sha256 = sha256_hex(&raw);
    let sav = sav::load(&raw)
        .map_err(|error| format!("save inválido {}: {error}", args.save.display()))?;
    let save_version = sav.version;
    let tick = sav.game_time.map(|time| time.tick);
    let climate = climate_code(sav.climate);
    let source_path = canonical_source_path(&args.save).display().to_string();
    let emitted = match args.stage {
        Stage::SavMap => dump_map(
            &sav.map,
            args,
            source_path,
            save_sha256,
            save_version,
            tick,
            climate,
        )?,
        Stage::GameStateMap => {
            let state = GameState::from_sav_game(sav);
            dump_map(
                &state.map,
                args,
                source_path,
                save_sha256,
                save_version,
                tick,
                climate,
            )?
        }
    };
    println!(
        "world-semantic {}: {} teselas ({}) → {}",
        args.stage.as_str(),
        emitted,
        args.save.display(),
        args.out.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("error: {error}");
            print_usage();
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
