//! Inspecciona bloques Action5 de un GRF, incluidos los sprites que logró decodificar.
//!
//! Es una utilidad de diagnóstico para mantener la correspondencia entre los
//! slots runtime de OpenTTD y los assets estáticos del cliente. No modifica el
//! GRF ni la partida.

use std::path::PathBuf;
use std::process::ExitCode;

use openttdrs_core::collect_action5_blocks;

struct Args {
    grf: PathBuf,
    type_filter: Option<u8>,
    sprites: bool,
}

fn usage() {
    eprintln!(
        "uso: action5_dumper <archivo.grf> [--type HEX] [--sprites]\n\
         \n\
         --type HEX  limita la salida a un Action5 (por ejemplo 06)\n\
         --sprites   imprime dimensiones y offsets de cada slot decodificado"
    );
}

fn parse_type(raw: &str) -> Result<u8, String> {
    u8::from_str_radix(raw.trim_start_matches("0x"), 16)
        .map_err(|error| format!("--type inválido {raw:?}: {error}"))
}

fn parse_args() -> Result<Args, String> {
    let mut grf = None;
    let mut type_filter = None;
    let mut sprites = false;
    let mut values = std::env::args().skip(1);

    while let Some(value) = values.next() {
        match value.as_str() {
            "--help" | "-h" => {
                usage();
                std::process::exit(0);
            }
            "--type" => {
                if type_filter.is_some() {
                    return Err("--type fue indicado más de una vez".to_string());
                }
                let raw = values
                    .next()
                    .ok_or_else(|| "falta el valor de --type".to_string())?;
                type_filter = Some(parse_type(&raw)?);
            }
            "--sprites" => sprites = true,
            value if value.starts_with('-') => return Err(format!("opción desconocida: {value}")),
            value if grf.is_none() => grf = Some(PathBuf::from(value)),
            value => return Err(format!("argumento inesperado: {value}")),
        }
    }

    Ok(Args {
        grf: grf.ok_or_else(|| "falta <archivo.grf>".to_string())?,
        type_filter,
        sprites,
    })
}

fn run(args: &Args) -> Result<(), String> {
    let data = std::fs::read(&args.grf)
        .map_err(|error| format!("no se pudo leer {}: {error}", args.grf.display()))?;
    let blocks = collect_action5_blocks(&data)
        .map_err(|error| format!("no se pudo parsear {}: {error}", args.grf.display()))?;

    let mut emitted = 0usize;
    for block in blocks.into_iter().filter(|block| {
        args.type_filter
            .is_none_or(|wanted| block.type_id == wanted)
    }) {
        emitted += 1;
        println!(
            "type=0x{:02X} offset={} declared={} decoded={}",
            block.type_id,
            block.offset,
            block.num_sprites,
            block.sprites.len()
        );
        if args.sprites {
            for (index, sprite) in block.sprites.iter().enumerate() {
                let slot = usize::from(block.offset) + index;
                println!(
                    "  slot={slot:>3} size={}x{} offset={},{} rgba={} mask={}",
                    sprite.width,
                    sprite.height,
                    sprite.x_offs,
                    sprite.y_offs,
                    sprite.rgba.len(),
                    sprite.mask.len()
                );
            }
        }
    }
    if emitted == 0 {
        return Err(match args.type_filter {
            Some(type_id) => format!("no hay bloques Action5 tipo 0x{type_id:02X}"),
            None => "el GRF no contiene bloques Action5 decodificables".to_string(),
        });
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse_args().and_then(|args| run(&args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("action5_dumper: {error}");
            usage();
            ExitCode::from(2)
        }
    }
}
