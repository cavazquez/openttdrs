#![allow(clippy::expect_used)]
//! Vuelca los datos crudos de teselas de un `.ottdmap` (mismo contenido que `Tile` en `map.rs`).
//!
//! Uso:
//! ```text
//! cargo run -p openttdrs-core --example dump_map_tiles -- /ruta/mapa.ottdmap 160 232 145 213
//! ```
//!
//! Muestra dimensiones W×H y comprueba que la rotación 180° en el plano de teselas cumple
//! `x' = (W-1) - x`, `y' = (H-1) - y` (OpenTTD: índices de celda en el array del mapa).

use openttdrs_core::{Map, TileCoord};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Uso: dump_map_tiles <mapa.ottdmap> [tx ty ...]");
        std::process::exit(1);
    }
    let path = &args[0];
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("leer {path}: {e}"));
    let map = Map::from_ottd_binary(&data).expect("MAP1 / formato válido");
    let (mw, mh) = map.dimensions();
    println!("Archivo: {path}");
    println!(
        "Dimensiones mapa: W={mw} H={mh} (teselas x ∈ 0..{}, y ∈ 0..{})\n",
        mw - 1,
        mh - 1
    );

    println!("Rotación 180° en índices de tesela (como espejo respecto al centro del rectángulo):");
    println!("  (x', y') = ((W-1) - x, (H-1) - y)");
    let rx = mw - 1 - 145;
    let ry = mh - 1 - 213;
    println!(
        "  (145, 213) → ({rx}, {ry})   [coincide con (160,232) solo si el mapa es {mw}×{mh}]\n"
    );

    let mut coords: Vec<(u32, u32)> = Vec::new();
    let mut i = 1;
    while i + 1 < args.len() {
        let tx: u32 = args[i].parse().expect("tx entero sin signo");
        let ty: u32 = args[i + 1].parse().expect("ty entero sin signo");
        coords.push((tx, ty));
        i += 2;
    }

    if coords.is_empty() {
        println!("(Sin pares tx ty; solo dimensiones y transformación de ejemplo.)");
        return;
    }

    for (tx, ty) in coords {
        println!("──────── tesela ({tx}, {ty}) ────────");
        match map.get(TileCoord::new(tx as i32, ty as i32)) {
            Some(t) => print_tile(&t),
            None => println!("  FUERA DE MAPA"),
        }
        println!();
    }
}

fn print_tile(t: &openttdrs_core::Tile) {
    let ottd_type = (t.mapt >> 4) & 0xF;
    println!("  kind (derivado MAPT): {:?}", t.kind);
    println!("  height (MAPH):       {}", t.height);
    println!(
        "  mapt (raw):          0x{:02x}  → TileType nibble: {ottd_type} ({})",
        t.mapt,
        ottd_type_name(ottd_type)
    );
    println!("  m5:                  0x{:02x}", t.m5);
    println!("  m1:                  0x{:02x}", t.m1);
    println!("  m6:                  0x{:02x}", t.m6);
    println!("  m8:                  0x{:04x}", t.m8);
    println!("  m3 (M3LO, v4+):      0x{:02x}", t.m3);
    println!("  m2 (MAP2, v5+):     0x{:02x}", t.m2);
    println!("  m7 (MAP7, v5+):     0x{:02x}", t.m7);
    println!("  m3hi (M3HI, v5+):   0x{:02x}", t.m3hi);
    if matches!(t.kind, openttdrs_core::TileKind::Water) {
        let wtt = (t.m5 >> 4) & 0x0F;
        println!("  WaterTileType (m5>>4): {wtt} (0=Clear, 1=Coast, …)");
    }
}

fn ottd_type_name(n: u8) -> &'static str {
    match n {
        0 => "MP_CLEAR",
        1 => "MP_RAILWAY",
        2 => "MP_ROAD",
        3 => "MP_HOUSE",
        4 => "MP_TREES",
        5 => "MP_STATION",
        6 => "MP_WATER",
        7 => "MP_VOID",
        8 => "MP_INDUSTRY",
        9 => "MP_TUNNELBRIDGE",
        10 => "MP_OBJECT",
        _ => "otro",
    }
}
