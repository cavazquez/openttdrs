//! Valida footer TNBP de un `.ottdmap` (p. ej. generado con `scripts/parse_sav.py` desde un `.sav` JGR).
//!
//! Uso: `cargo run -p openttdrs-core --example validate_ottdmap_tnbp -- ruta.ottdmap`

use openttdrs_core::{Map, decode_tnbp_blob, jgr_tunnels_from_decoded, tnbp_blob_to_json_value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("uso: validate_ottdmap_tnbp <archivo.ottdmap>")?;
    let data = std::fs::read(&path)?;
    let (map, ex) =
        Map::from_ottd_binary_with_extras(&data).map_err(|e| format!("mapa inválido: {e:?}"))?;
    let (w, h) = map.dimensions();
    println!("Mapa: {w}×{h}");
    let len = ex.tnbp_blob_len();
    println!("TNBP: {len} bytes en footer");
    if len == 0 {
        println!("(sin blob TNBP — save vanilla o sin chunk TUNN/TNBP/TBUS)");
        return Ok(());
    }
    let Some(blob) = ex.tnbp_blob.as_deref() else {
        return Err("TNBP anunciado pero blob ausente".into());
    };
    match decode_tnbp_blob(blob) {
        Ok(dec) => {
            let jgr = jgr_tunnels_from_decoded(&dec);
            let (n_ok, s_ok, tot) = map.jgr_tunnel_endpoint_match_stats(&jgr);
            println!(
                "Túneles JGR: {tot} registro(s); extremos en MP_TUNNELBRIDGE: norte {n_ok}/{tot}, sur {s_ok}/{tot}"
            );
            if let openttdrs_core::TnbpDecoded::ChTable { skipped_rows, .. } = &dec
                && *skipped_rows > 0
            {
                println!("  (filas Sl omitidas por versión/campos: {skipped_rows})");
            }
        }
        Err(e) => println!("Decode TNBP: Err({e:?})"),
    }
    let summary = ex
        .tnbp_blob
        .as_deref()
        .map(tnbp_blob_to_json_value)
        .unwrap_or(serde_json::json!({}));
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
