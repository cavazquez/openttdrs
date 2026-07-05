//! Sistema de paridad contra `OpenTTD`: trazas por tick, escenarios headless,
//! comparador de primera divergencia y reporte de divergencias conocidas.
//!
//! Flujo típico:
//!
//! ```
//! use openttdrs_core::parity;
//!
//! let mut state = parity::build_scenario("truck_bay").unwrap();
//! state.enable_parity_trace();
//! for _ in 0..50 {
//!     state.step();
//! }
//! let records = state.take_parity_records();
//! assert_eq!(records.len(), 50);
//! ```

mod diff;
mod record;
pub mod report;
mod scenario;
mod tracer;

pub use diff::{DiffFilter, DiffReport, Divergence, Subsystem, compare_traces, render_report};
pub use record::{
    ParityEvent, RailPartRecord, RailRecord, SpeedTrend, TickRecord, TraceVehicleState,
    VehicleRecord, derive_vehicle_state, order_kind_name,
};
pub use scenario::{
    TRAIN_LINE_CORNER, TRAIN_LINE_DEPOT, TRAIN_LINE_SIGNAL, TRAIN_LINE_STATION_A,
    TRAIN_LINE_STATION_B, TRAIN_LINE_VEHICLE_ID, TRAIN_SIGNAL_BLOCK_TILE, TRAIN_SIGNAL_BLOCKER_ID,
    TRAIN_SIGNAL_LEAD_ID, TRAIN_SIGNAL_TILE, TRUCK_BAY_DELIVER_ROAD, TRUCK_BAY_DELIVER_STOP,
    TRUCK_BAY_LOAD_ROAD, TRUCK_BAY_LOAD_STOP, TRUCK_BAY_VEHICLE_ID, build_scenario,
    build_train_line, build_train_signal, build_truck_bay, scenario_names,
};
pub use tracer::ParityTracer;

pub(crate) use tracer::record_tick;

use std::io::{BufRead, Write};

/// Serializa registros a JSONL (una línea JSON por tick).
///
/// # Errors
///
/// Propaga errores de E/S del `writer` o de serialización.
pub fn write_jsonl<W: Write>(records: &[TickRecord], writer: &mut W) -> std::io::Result<()> {
    for record in records {
        let line = serde_json::to_string(record).map_err(std::io::Error::other)?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

/// Lee registros desde JSONL (ignora líneas vacías).
///
/// # Errors
///
/// Propaga errores de E/S del `reader` o de parseo JSON.
pub fn read_jsonl<R: BufRead>(reader: R) -> std::io::Result<Vec<TickRecord>> {
    let mut records = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record: TickRecord = serde_json::from_str(&line).map_err(std::io::Error::other)?;
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_roundtrip_preserves_records() {
        let mut state = build_truck_bay();
        state.enable_parity_trace();
        for _ in 0..10 {
            state.step();
        }
        let records = state.take_parity_records();
        let mut buf = Vec::new();
        write_jsonl(&records, &mut buf).unwrap();
        let parsed = read_jsonl(std::io::Cursor::new(buf)).unwrap();
        assert_eq!(records, parsed);
    }
}
