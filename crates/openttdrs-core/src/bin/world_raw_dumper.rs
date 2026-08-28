//! Exporta bytes crudos de todas las teselas de un `.sav` para la paridad #305.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use openttdrs_core::sav;
use openttdrs_core::world_raw::{
    WorldRawContext, WorldRawMetadata, WorldRawRegion, sha256_hex, write_world_raw_jsonl,
};
use openttdrs_core::{
    Climate, GameState, Map, PopulationGenConfig, TerrainType, WorldGenConfig,
    apply_population_gen_with_rng, apply_world_gen_with_rng, generate_objects_with_rng,
    generate_trees_with_rng, run_generation_tile_loop,
};

#[derive(Debug, Clone, Copy)]
enum Stage {
    SavMap,
    GameStateMap,
    TreesReplay,
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
            Self::TreesReplay => "trees_replay",
        }
    }
}

struct Args {
    save: Option<PathBuf>,
    out: PathBuf,
    generate: Option<(u32, u32)>,
    replay_trees: bool,
    seed: Option<u64>,
    stage: Stage,
    region: Option<WorldRawRegion>,
    openttd_commit: String,
}

fn print_usage() {
    eprintln!(
        "uso: world_raw_dumper <partida.sav> <salida.jsonl> [opciones]\n\
         o:   world_raw_dumper --generate WIDTHxHEIGHT --seed N <salida.jsonl> [opciones]\n\
         o:   world_raw_dumper --replay-trees <pre-arboles.sav> <salida.jsonl> [opciones]\n\
         \n\
         Opciones:\n\
           --generate WIDTHxHEIGHT          genera el mapa procedural openttdrs (sin guardar .sav)\n\
           --seed N                          semilla para --generate\n\
           --replay-trees                    reproduce GenerateTrees desde DATE.random_state\n\
           --stage sav_map|game_state_map  etapa a exportar (default: sav_map)\n\
           --region x0,y0,x1,y1            rectángulo inclusivo de teselas\n\
           --tile x,y [--radius N]          tesela, opcionalmente con contexto\n\
           --openttd-commit SHA             manifiesto del oráculo para metadata\n\
         \n\
         `--generate` incluye pueblos e industrias por defecto; usar\n\
         `OPENTTDRS_GENERATE_POPULATION=0` para omitir población y conservar\n\
         objetos/árboles del flujo de generación.\n\
         `OPENTTDRS_GENERATE_STARTUP_TICKS=N` reproduce N ciclos de\n\
         `RunTileLoop` posteriores a la generación (OpenTTD usa 1280).\n\
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

fn parse_dimensions(value: &str) -> Result<(u32, u32), String> {
    let mut values = value.split('x');
    let width = values
        .next()
        .ok_or_else(|| "--generate debe tener formato WIDTHxHEIGHT".to_string())?;
    let height = values
        .next()
        .ok_or_else(|| "--generate debe tener formato WIDTHxHEIGHT".to_string())?;
    if values.next().is_some() {
        return Err("--generate debe tener exactamente WIDTHxHEIGHT".to_string());
    }
    let width = parse_u32(width, "--generate")?;
    let height = parse_u32(height, "--generate")?;
    if !(64..=4096).contains(&width)
        || !(64..=4096).contains(&height)
        || !width.is_power_of_two()
        || !height.is_power_of_two()
    {
        return Err("--generate admite dimensiones potencia de dos entre 64 y 4096".to_string());
    }
    Ok((width, height))
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
    let mut generate = None;
    let mut seed = None;
    let mut stage = None;
    let mut replay_trees = false;
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
            "--generate" => {
                if generate.is_some() {
                    return Err("--generate fue indicado más de una vez".to_string());
                }
                generate = Some(parse_dimensions(&next_value(&mut args, "--generate")?)?);
            }
            "--replay-trees" => {
                if replay_trees {
                    return Err("--replay-trees fue indicado más de una vez".to_string());
                }
                replay_trees = true;
            }
            "--seed" => {
                if seed.is_some() {
                    return Err("--seed fue indicado más de una vez".to_string());
                }
                seed = Some(
                    next_value(&mut args, "--seed")?
                        .parse::<u64>()
                        .map_err(|error| format!("--seed inválido: {error}"))?,
                );
            }
            "--stage" => {
                if stage.is_some() {
                    return Err("--stage fue indicado más de una vez".to_string());
                }
                stage = Some(Stage::parse(&next_value(&mut args, "--stage")?)?);
            }
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

    if generate.is_some() {
        if replay_trees {
            return Err("--generate y --replay-trees no pueden usarse juntos".to_string());
        }
        if positional.len() != 1 {
            return Err("--generate requiere sólo <salida.jsonl>".to_string());
        }
        if seed.is_none() {
            return Err("--generate requiere --seed N".to_string());
        }
    } else if positional.len() != 2 {
        return Err("se requieren <partida.sav> y <salida.jsonl>".to_string());
    } else if seed.is_some() {
        return Err("--seed sólo se puede usar con --generate".to_string());
    }
    let stage = if replay_trees {
        if stage.is_some() {
            return Err("--stage no se puede usar con --replay-trees".to_string());
        }
        Stage::TreesReplay
    } else {
        stage.unwrap_or(Stage::SavMap)
    };
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
        save: (generate.is_none()).then(|| positional.remove(0)),
        out: positional.remove(0),
        generate,
        replay_trees,
        seed,
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
    let context = WorldRawContext {
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
    let metadata = WorldRawMetadata::for_map(map, &context);
    let file = File::create(&args.out)
        .map_err(|error| format!("no se pudo crear {}: {error}", args.out.display()))?;
    let mut writer = BufWriter::new(file);
    let summary = write_world_raw_jsonl(&mut writer, &metadata, map)
        .map_err(|error| format!("no se pudo escribir {}: {error}", args.out.display()))?;
    writer
        .flush()
        .map_err(|error| format!("no se pudo cerrar {}: {error}", args.out.display()))?;
    Ok(summary.emitted_tile_count)
}

fn run(args: &Args) -> Result<(), String> {
    if let Some((width, height)) = args.generate {
        let Some(seed) = args.seed else {
            return Err("--generate requiere --seed N".to_string());
        };
        let mut map = Map::new_flat(width, height, 0);
        let config = WorldGenConfig {
            climate: Climate::Temperate,
            seed,
            sea_level: 1,
            // Defaults equivalentes al generador TGP de OpenTTD: relieve
            // Flat (1), nivel de mar muy bajo y `water_borders=Random`.
            // Los bordes void se materializan en `apply_world_gen`, como hace
            // `freeform_edges` al inicializar el mapa C++.
            island: false,
            water_borders: Some(
                std::env::var("OPENTTDRS_WATER_BORDERS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0x10),
            ),
            amount_of_rivers: std::env::var("OPENTTDRS_AMOUNT_OF_RIVERS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(2),
            startup_rng_draws: std::env::var("OPENTTDRS_STARTUP_RNG_DRAWS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(1),
            ..WorldGenConfig::default().with_terrain_type(TerrainType::Flat)
        };
        let mut generation_rng =
            apply_world_gen_with_rng(&mut map, &config, &[]).map_err(|error| {
                format!("falló la generación {width}x{height}, seed={seed}: {error:?}")
            })?;
        // Una partida nueva de OpenTTD continúa con pueblos e industrias
        // después del paisaje. Mantener esta etapa activada por defecto hace
        // que `--generate` represente un mapa jugable; `...=0` conserva el
        // contrato útil para aislar únicamente el terreno.
        let generate_population = std::env::var("OPENTTDRS_GENERATE_POPULATION")
            .ok()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(1)
            != 0;
        let startup_ticks = std::env::var("OPENTTDRS_GENERATE_STARTUP_TICKS")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let mut state = GameState::from_map(map);
        state.world_seed = seed;
        state.climate = Climate::Temperate;
        if generate_population {
            // OpenTTD ejecuta GenerateTowns/GenerateIndustries antes del
            // primer ciclo de RunTileLoop. Mantenerlo opcional permite
            // aislar el relieve en la matriz de diagnóstico.
            apply_population_gen_with_rng(
                &mut state,
                &PopulationGenConfig {
                    seed,
                    ..PopulationGenConfig::default()
                },
                &[],
                &mut generation_rng,
            );
        } else {
            // Incluso en el modo de diagnóstico "sólo terreno", OpenTTD
            // ejecuta `GenerateTrees` antes de entregar el mundo nuevo. La
            // separación conserva la posibilidad de aislar pueblos/industrias
            // sin convertir el raw en un mapa artificialmente pelado.
            let climate = state.climate;
            generate_objects_with_rng(&mut state, climate, &mut generation_rng, &[]);
            generate_trees_with_rng(&mut state.map, state.climate, &mut generation_rng, &[]);
        }
        // El RNG global que usa `RunTileLoop` es el mismo stream que dejó
        // `GenerateTrees`; reiniciar el estado a la semilla 1 aquí desfasaba
        // industria/árboles y cualquier callback que consuma Random().
        state.random = generation_rng;
        for tick in 0..u64::from(startup_ticks) {
            run_generation_tile_loop(&mut state, tick);
        }
        map = state.map;
        let source = format!("generated:{width}x{height}:seed={seed}");
        let emitted = dump_map(
            &map,
            args,
            source.clone(),
            sha256_hex(source.as_bytes()),
            0,
            (startup_ticks != 0).then_some(u64::from(startup_ticks) + 1),
            0,
        )?;
        println!(
            "world-raw {}: {} teselas ({source}) → {}",
            args.stage.as_str(),
            emitted,
            args.out.display()
        );
        return Ok(());
    }
    let Some(save) = args.save.as_ref() else {
        return Err("se requiere <partida.sav> cuando no se usa --generate".to_string());
    };
    let raw = std::fs::read(save)
        .map_err(|error| format!("no se pudo leer {}: {error}", save.display()))?;
    let save_sha256 = sha256_hex(&raw);
    let source_path = canonical_source_path(save).display().to_string();
    if raw.starts_with(b"MAP1") {
        let map = Map::from_ottd_binary(&raw)
            .map_err(|error| format!("ottdmap inválido {}: {error:?}", save.display()))?;
        let emitted = dump_map(&map, args, source_path, save_sha256, 0, None, 0)?;
        println!(
            "world-raw {}: {} teselas ({}) → {}",
            args.stage.as_str(),
            emitted,
            save.display(),
            args.out.display()
        );
        return Ok(());
    }
    let sav =
        sav::load(&raw).map_err(|error| format!("save inválido {}: {error}", save.display()))?;
    let save_version = sav.version;
    let tick = sav.game_time.map(|time| time.tick);
    let climate = climate_code(sav.climate);
    if args.replay_trees {
        let random_state = sav.random_state.ok_or_else(|| {
            format!(
                "{} no contiene DATE.random_state[0..1]; no se puede reproducir GenerateTrees",
                save.display()
            )
        })?;
        let mut map = sav.map;
        let mut rng = openttdrs_core::linkgraph_parity::Randomizer {
            state: random_state,
        };
        generate_trees_with_rng(&mut map, sav.climate, &mut rng, &[]);
        let source_path = format!("trees-replay:{source_path}");
        let emitted = dump_map(
            &map,
            args,
            source_path,
            save_sha256,
            save_version,
            tick,
            climate,
        )?;
        println!(
            "world-raw {}: {} teselas ({}) → {} [rng {:08x},{:08x} → {:08x},{:08x}]",
            args.stage.as_str(),
            emitted,
            save.display(),
            args.out.display(),
            random_state[0],
            random_state[1],
            rng.state[0],
            rng.state[1],
        );
        return Ok(());
    }
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
        Stage::TreesReplay => unreachable!("--replay-trees se maneja antes"),
    };
    println!(
        "world-raw {}: {} teselas ({}) → {}",
        args.stage.as_str(),
        emitted,
        save.display(),
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

#[cfg(test)]
mod tests {
    use super::parse_dimensions;

    #[test]
    fn generated_dimensions_accept_openttd_sizes() {
        assert_eq!(parse_dimensions("64x128"), Ok((64, 128)));
        assert!(parse_dimensions("32x64").is_err());
        assert!(parse_dimensions("96x64").is_err());
        assert!(parse_dimensions("64x64x64").is_err());
    }
}
