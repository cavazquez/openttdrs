//! Opt-in frame timings for a real window/GPU, without kernel perf permissions.
//! `OPENTTDRS_PERF_OUT=file.csv`; optional `OPENTTDRS_PERF_FRAMES=N` exits
//! after N samples, following `OPENTTDRS_PERF_WARMUP` frames (default 120). Paused/running comparisons
//! use `OPENTTDRS_PERF_PAUSED=1`; `OPENTTDRS_PERF_SCALE` fixes the camera zoom.
//! `OPENTTDRS_PERF_PAN=1` moves the camera during the measured frames.
//! `OPENTTDRS_PERF_MAIN_OUT=file.csv` adds opt-in wall timings around every
//! schedule currently in Bevy's main loop, alongside the normal capture.

mod main_schedule;
pub(crate) mod remap;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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

struct FrameCapture {
    output: BufWriter<File>,
    path: PathBuf,
    main_detail: Option<main_schedule::MainScheduleCapture>,
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

// Keep diagnostic state outside ECS: early resource/query registration changes
// scene producer iteration and can change the raster at equal projected depths.
type SharedFrameCapture = Arc<Mutex<FrameCapture>>;

pub(crate) struct PerformancePlugin;

impl Plugin for PerformancePlugin {
    fn build(&self, app: &mut App) {
        let Some(path) = std::env::var_os("OPENTTDRS_PERF_OUT") else {
            return;
        };
        let result = File::create(&path).and_then(|file| {
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
        let capture = Arc::new(Mutex::new(FrameCapture {
            output,
            path: path.into(),
            main_detail: None,
            frame: 0,
            warmup,
            limit,
            previous: now,
            start: now,
            frame_time: Duration::ZERO,
            scale,
            pan: std::env::var("OPENTTDRS_PERF_PAN").is_ok_and(|value| value == "1"),
            pan_origin: None,
        }));
        ENABLED.store(true, Ordering::Relaxed);
        install_frame_systems(app, &capture);
        main_schedule::install_from_env(app, &capture);
    }
}

fn install_frame_systems(app: &mut App, capture: &SharedFrameCapture) {
    let begin_capture = Arc::clone(capture);
    app.add_systems(First, move |world: &mut World| {
        if let Ok(mut capture) = begin_capture.lock() {
            begin_frame(&mut capture, world);
        }
    });
    let finish_capture = Arc::clone(capture);
    app.add_systems(Last, move |world: &mut World| {
        if let Ok(mut capture) = finish_capture.lock() {
            finish_frame(&mut capture, world);
        }
    });
}

fn begin_frame(capture: &mut FrameCapture, world: &mut World) {
    let now = Instant::now();
    capture.frame += 1;
    capture.frame_time = now - capture.previous;
    capture.previous = now;
    capture.start = now;
    if let Ok(mut timings) = TIMINGS.lock() {
        *timings = [Duration::ZERO; PHASE_COUNT];
    }
    let set_scale = capture.frame == 30 && capture.scale.is_some();
    let pan = capture.pan && capture.frame > capture.warmup;
    if !set_scale && !pan {
        return;
    }
    let mut cameras = world.query_filtered::<
        (&mut Projection, &mut Transform),
        (With<PrimaryGameCamera>, Without<MapPreviewCamera>),
    >();
    if set_scale
        && let Some(scale) = capture.scale
        && let Ok((mut projection, _)) = cameras.single_mut(world)
        && let Projection::Orthographic(ortho) = &mut *projection
    {
        ortho.scale = scale;
    }
    if pan && let Ok((_, mut transform)) = cameras.single_mut(world) {
        let origin = *capture.pan_origin.get_or_insert(transform.translation);
        let angle = (capture.frame - capture.warmup) as f32 * std::f32::consts::TAU / 360.0;
        transform.translation =
            origin + Vec3::new(256.0 * (angle.cos() - 1.0), 256.0 * angle.sin(), 0.0);
    }
}

fn finish_frame(capture: &mut FrameCapture, world: &mut World) {
    let sample = capture.frame.saturating_sub(capture.warmup);
    if sample == 0 {
        return;
    }
    let update_time = capture.start.elapsed().as_secs_f64() * 1000.0;
    let frame_time = capture.frame_time.as_secs_f64() * 1000.0;
    let timings = TIMINGS.lock().map(|timings| *timings).unwrap_or_default();
    let tick = world.resource::<SimWorld>().state.tick.get();
    // Register counter queries only after the scene has completed warmup.
    let sprites = world.query_filtered::<(), With<Sprite>>().iter(world).len();
    let meshes = world
        .query_filtered::<(), With<SpriteMesh>>()
        .iter(world)
        .len();
    let result = (|| -> std::io::Result<()> {
        write!(capture.output, "{sample},{frame_time:.4},{update_time:.4}")?;
        for duration in timings {
            write!(capture.output, ",{:.4}", duration.as_secs_f64() * 1000.0)?;
        }
        writeln!(capture.output, ",{tick},{sprites},{meshes}")?;
        if sample.is_multiple_of(60) || capture.limit == Some(sample) {
            capture.output.flush()?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        error!("Cannot write performance capture: {error}");
        world.write_message(AppExit::error());
    } else if capture.limit == Some(sample) {
        world.write_message(AppExit::Success);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use openttdrs_core::GameState;

    #[test]
    fn capture_matches_uninstrumented_registry_during_warmup() {
        let path = std::env::temp_dir().join(format!(
            "openttdrs-frame-registry-{}.csv",
            std::process::id()
        ));
        let now = Instant::now();
        let capture = Arc::new(Mutex::new(FrameCapture {
            output: BufWriter::new(File::create(&path).expect("fixture output")),
            path: path.clone(),
            main_detail: None,
            frame: 0,
            warmup: 2,
            limit: Some(1),
            previous: now,
            start: now,
            frame_time: Duration::ZERO,
            scale: None,
            pan: false,
            pan_origin: None,
        }));
        let mut app = App::new();
        let mut control = App::new();
        control.add_systems(First, |_: &mut World| {});
        control.add_systems(Last, |_: &mut World| {});
        let components = app.world().components().len();
        install_frame_systems(&mut app, &capture);
        assert_eq!(app.world().components().len(), components);
        for _ in 0..2 {
            control.world_mut().run_schedule(First);
            control.world_mut().run_schedule(Last);
            app.world_mut().run_schedule(First);
            app.world_mut().run_schedule(Last);
            let registry = |app: &App| {
                app.world()
                    .components()
                    .iter_registered()
                    .map(|info| info.name().to_string())
                    .collect::<Vec<_>>()
            };
            assert_eq!(registry(&app), registry(&control));
        }
        assert!(
            app.world()
                .components()
                .get_id(std::any::TypeId::of::<Sprite>())
                .is_none()
        );
        assert!(
            app.world()
                .components()
                .get_id(std::any::TypeId::of::<SpriteMesh>())
                .is_none()
        );
        assert!(
            app.world()
                .components()
                .get_id(std::any::TypeId::of::<Projection>())
                .is_none()
        );
        assert!(
            std::fs::read_to_string(&path)
                .expect("warmup output")
                .is_empty()
        );

        app.insert_resource(SimWorld {
            state: GameState::new(4, 4),
            loaded_file: false,
            ottdmap_extras: None,
        });
        app.world_mut().spawn(Sprite::default());
        app.world_mut().spawn(Sprite::default());
        app.world_mut().spawn(SpriteMesh::default());
        let components = app.world().components().len();
        app.world_mut().run_schedule(First);
        app.world_mut().run_schedule(Last);
        assert_eq!(app.world().components().len(), components);
        let content = std::fs::read_to_string(&path).expect("flushed sample");
        let fields: Vec<_> = content.trim().split(',').collect();
        assert_eq!(fields.len(), 15);
        assert_eq!(fields[0], "1");
        assert_eq!(&fields[12..], &["0", "2", "1"]);
        assert!(
            app.world()
                .components()
                .get_id(std::any::TypeId::of::<Projection>())
                .is_none()
        );
        drop(app);
        drop(capture);
        std::fs::remove_file(path).expect("fixture cleanup");
    }
}
