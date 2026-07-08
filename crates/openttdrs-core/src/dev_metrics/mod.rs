//! Medición headless para desarrollo y QA (carga → descarga → ingresos).
//!
//! Módulo **opcional**: no participa en la simulación del juego. Para quitarlo por
//! completo, borrar el directorio `dev_metrics/`, `src/bin/dev_bot.rs` y la línea
//! `pub mod dev_metrics` en `lib.rs`.

mod cargo_probe;
mod signal_probe;

pub use cargo_probe::{CargoProbeOptions, VehicleCargoReport, probe_vehicle_cargo_cycle};
pub use signal_probe::{SignalWaitProbeOptions, SignalWaitReport, probe_signal_wait};
