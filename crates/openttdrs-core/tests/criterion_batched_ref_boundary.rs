//! Contrato de lifetime de Criterion usado por los benches de pathfinding.

#![allow(clippy::expect_used)] // el temporal es un fixture del test

use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use criterion::measurement::{Measurement, ValueFormatter};
use criterion::{BatchSize, Criterion, Throughput};

/// Formatter mínimo: el test sólo inspecciona la frontera `start`/`end`.
struct BoundaryFormatter;

impl ValueFormatter for BoundaryFormatter {
    fn scale_values(&self, _typical_value: f64, _values: &mut [f64]) -> &'static str {
        "ns"
    }

    fn scale_throughputs(
        &self,
        _typical_value: f64,
        _throughput: &Throughput,
        _values: &mut [f64],
    ) -> &'static str {
        "ns"
    }

    fn scale_for_machines(&self, _values: &mut [f64]) -> &'static str {
        "ns"
    }
}

static BOUNDARY_FORMATTER: BoundaryFormatter = BoundaryFormatter;

/// `Measurement` que señala exactamente el intervalo que Criterion considera
/// cronometrado, preservando la métrica de pared para que el runner complete
/// sus muestras normales.
struct BoundaryMeasurement {
    measuring: Arc<AtomicBool>,
}

impl Measurement for BoundaryMeasurement {
    type Intermediate = Instant;
    type Value = Duration;

    fn start(&self) -> Self::Intermediate {
        self.measuring.store(true, Ordering::SeqCst);
        Instant::now()
    }

    fn end(&self, start: Self::Intermediate) -> Self::Value {
        let elapsed = start.elapsed();
        self.measuring.store(false, Ordering::SeqCst);
        elapsed
    }

    fn add(&self, first: &Self::Value, second: &Self::Value) -> Self::Value {
        *first + *second
    }

    fn zero(&self) -> Self::Value {
        Duration::ZERO
    }

    fn to_f64(&self, value: &Self::Value) -> f64 {
        value.as_nanos() as f64
    }

    fn formatter(&self) -> &dyn ValueFormatter {
        &BOUNDARY_FORMATTER
    }
}

struct DropProbe {
    measuring: Arc<AtomicBool>,
    dropped_during_measurement: Arc<AtomicUsize>,
    dropped_total: Arc<AtomicUsize>,
}

impl DropProbe {
    fn new(
        measuring: Arc<AtomicBool>,
        dropped_during_measurement: Arc<AtomicUsize>,
        dropped_total: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            measuring,
            dropped_during_measurement,
            dropped_total,
        }
    }
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.dropped_total.fetch_add(1, Ordering::SeqCst);
        if self.measuring.load(Ordering::SeqCst) {
            self.dropped_during_measurement
                .fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[test]
fn criterion_batched_ref_keeps_input_and_output_drops_outside_measurement() {
    let measuring = Arc::new(AtomicBool::new(false));
    let dropped_during_measurement = Arc::new(AtomicUsize::new(0));
    let dropped_total = Arc::new(AtomicUsize::new(0));
    let output = tempfile::tempdir().expect("directorio Criterion temporal");
    let mut criterion = Criterion::default()
        .without_plots()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(1))
        .measurement_time(Duration::from_millis(1))
        .nresamples(10)
        .output_directory(output.path())
        .with_measurement(BoundaryMeasurement {
            measuring: Arc::clone(&measuring),
        });

    let setup_measuring = Arc::clone(&measuring);
    let setup_during = Arc::clone(&dropped_during_measurement);
    let setup_total = Arc::clone(&dropped_total);
    let routine_measuring = Arc::clone(&measuring);
    let routine_during = Arc::clone(&dropped_during_measurement);
    let routine_total = Arc::clone(&dropped_total);
    criterion.bench_function("iter_batched_ref_drop_boundary", |b| {
        b.iter_batched_ref(
            || {
                DropProbe::new(
                    Arc::clone(&setup_measuring),
                    Arc::clone(&setup_during),
                    Arc::clone(&setup_total),
                )
            },
            |_| {
                black_box(());
                DropProbe::new(
                    Arc::clone(&routine_measuring),
                    Arc::clone(&routine_during),
                    Arc::clone(&routine_total),
                )
            },
            BatchSize::SmallInput,
        );
    });

    assert!(
        dropped_total.load(Ordering::SeqCst) > 0,
        "probes ejecutados"
    );
    assert_eq!(
        dropped_during_measurement.load(Ordering::SeqCst),
        0,
        "Criterion 0.8 debe liberar input y output después de end()"
    );
}
