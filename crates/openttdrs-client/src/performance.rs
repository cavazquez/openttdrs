//! Opt-in frame timings for a real window/GPU, without kernel perf permissions.
//! `OPENTTDRS_PERF_OUT=file.csv`; optional `OPENTTDRS_PERF_FRAMES=N` exits
//! after N samples, following `OPENTTDRS_PERF_WARMUP` frames (default 120). Paused/running comparisons
//! use `OPENTTDRS_PERF_PAUSED=1`; `OPENTTDRS_PERF_SCALE` fixes the camera zoom.
//! `OPENTTDRS_PERF_PAN=1` moves the camera during the measured frames.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::sprite::SpriteMesh;

use crate::render::{MapPreviewCamera, PrimaryGameCamera};
use crate::simulation::VisualCaptureFreeze;
use crate::state::SimWorld;

const PHASE_COUNT: usize = 9;
static ENABLED: AtomicBool = AtomicBool::new(false);
static TIMINGS: Mutex<[Duration; PHASE_COUNT]> = Mutex::new([Duration::ZERO; PHASE_COUNT]);

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Simulation,
    Remap,
    Sort,
    Children,
    Glass,
    Vehicles,
    Water,
    Minimap,
    Effects,
}

pub(crate) struct Measurement {
    phase: Phase,
    start: Option<Instant>,
}

#[must_use]
pub(crate) fn measure(phase: Phase) -> Measurement {
    Measurement {
        phase,
        start: ENABLED.load(Ordering::Relaxed).then(Instant::now),
    }
}

impl Drop for Measurement {
    fn drop(&mut self) {
        if let Some(start) = self.start
            && let Ok(mut timings) = TIMINGS.lock()
        {
            timings[self.phase as usize] += start.elapsed();
        }
    }
}

#[derive(Resource)]
struct FrameCapture {
    output: BufWriter<File>,
    frame: u32,
    warmup: u32,
    limit: Option<u32>,
    previous: Instant,
    start: Instant,
    frame_time: Duration,
    scale: Option<f32>,
    pan: bool,
    pan_origin: Option<Vec3>,
}

pub(crate) struct PerformancePlugin;

impl Plugin for PerformancePlugin {
    fn build(&self, app: &mut App) {
        let Some(path) = std::env::var_os("OPENTTDRS_PERF_OUT") else {
            return;
        };
        let result = File::create(path).and_then(|file| {
            let mut output = BufWriter::new(file);
            writeln!(
                output,
                "frame,frame_ms,update_ms,simulation_ms,remap_ms,sort_ms,children_ms,glass_ms,vehicles_ms,water_ms,minimap_ms,effects_ms,tick,sprites,meshes"
            )?;
            output.flush()?;
            Ok(output)
        });
        let output = match result {
            Ok(output) => output,
            Err(error) => {
                error!("Cannot create performance capture: {error}");
                return;
            }
        };
        let now = Instant::now();
        let limit = std::env::var("OPENTTDRS_PERF_FRAMES")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|&value| value > 0);
        let scale = std::env::var("OPENTTDRS_PERF_SCALE")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value > 0.0);
        let warmup = std::env::var("OPENTTDRS_PERF_WARMUP")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(120);
        if std::env::var("OPENTTDRS_PERF_PAUSED").is_ok_and(|value| value == "1") {
            app.world_mut().resource_mut::<VisualCaptureFreeze>().0 = true;
        }
        app.insert_resource(FrameCapture {
            output,
            frame: 0,
            warmup,
            limit,
            previous: now,
            start: now,
            frame_time: Duration::ZERO,
            scale,
            pan: std::env::var("OPENTTDRS_PERF_PAN").is_ok_and(|value| value == "1"),
            pan_origin: None,
        });
        ENABLED.store(true, Ordering::Relaxed);
        app.add_systems(First, begin_frame);
        app.add_systems(Last, finish_frame);
    }
}

fn begin_frame(
    mut capture: ResMut<FrameCapture>,
    mut cameras: Query<
        (&mut Projection, &mut Transform),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >,
) {
    let now = Instant::now();
    capture.frame += 1;
    capture.frame_time = now - capture.previous;
    capture.previous = now;
    capture.start = now;
    if let Ok(mut timings) = TIMINGS.lock() {
        *timings = [Duration::ZERO; PHASE_COUNT];
    }
    if capture.frame == 30
        && let Some(scale) = capture.scale
        && let Ok((mut projection, _)) = cameras.single_mut()
        && let Projection::Orthographic(ortho) = &mut *projection
    {
        ortho.scale = scale;
    }
    if capture.pan
        && capture.frame > capture.warmup
        && let Ok((_, mut transform)) = cameras.single_mut()
    {
        let origin = *capture.pan_origin.get_or_insert(transform.translation);
        let angle = (capture.frame - capture.warmup) as f32 * std::f32::consts::TAU / 360.0;
        transform.translation =
            origin + Vec3::new(256.0 * (angle.cos() - 1.0), 256.0 * angle.sin(), 0.0);
    }
}

fn finish_frame(
    mut capture: ResMut<FrameCapture>,
    sim: Res<SimWorld>,
    sprites: Query<(), With<Sprite>>,
    meshes: Query<(), With<SpriteMesh>>,
    mut exit: MessageWriter<AppExit>,
) {
    let sample = capture.frame.saturating_sub(capture.warmup);
    if sample == 0 {
        return;
    }
    let update_time = capture.start.elapsed().as_secs_f64() * 1000.0;
    let frame_time = capture.frame_time.as_secs_f64() * 1000.0;
    let timings = TIMINGS.lock().map(|timings| *timings).unwrap_or_default();
    let result = (|| -> std::io::Result<()> {
        write!(capture.output, "{sample},{frame_time:.4},{update_time:.4}")?;
        for duration in timings {
            write!(capture.output, ",{:.4}", duration.as_secs_f64() * 1000.0)?;
        }
        writeln!(
            capture.output,
            ",{},{},{}",
            sim.state.tick.get(),
            sprites.iter().len(),
            meshes.iter().len()
        )?;
        if sample.is_multiple_of(60) || capture.limit == Some(sample) {
            capture.output.flush()?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        error!("Cannot write performance capture: {error}");
        exit.write(AppExit::error());
    } else if capture.limit == Some(sample) {
        exit.write(AppExit::Success);
    }
}
