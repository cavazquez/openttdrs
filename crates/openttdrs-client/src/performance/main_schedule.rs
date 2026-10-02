//! Optional schedule boundaries; existing schedules and systems keep their order.
//! These wall intervals include dispatch overhead, not render-thread or GPU work.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bevy::app::MainScheduleOrder;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

use super::SharedFrameCapture;
use crate::state::SimWorld;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct MainTimingStart;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct MainTimingBoundary(usize);

pub(super) struct MainScheduleCapture {
    output: BufWriter<File>,
    phases: Vec<Duration>,
    previous: Option<Instant>,
    start: Option<Instant>,
}

pub(super) fn install_from_env(app: &mut App, capture: &SharedFrameCapture) {
    let Some(path) = std::env::var_os("OPENTTDRS_PERF_MAIN_OUT") else {
        return;
    };
    if let Err(error) = install(app, Path::new(&path), capture) {
        error!("Cannot create main schedule capture: {error}");
    }
}

fn install(app: &mut App, path: &Path, capture: &SharedFrameCapture) -> io::Result<()> {
    let normal_path = capture
        .lock()
        .map_err(|_| io::Error::other("frame capture lock is poisoned"))?
        .path
        .clone();
    ensure_distinct_output(&normal_path, path)?;
    let original = app.world().resource::<MainScheduleOrder>().labels.clone();
    if original.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "main schedule order is empty",
        ));
    }
    let mut output = BufWriter::new(File::create(path)?);
    write!(output, "frame,tick,main_ms")?;
    for (index, label) in original.iter().enumerate() {
        let name = format!("main_{index:02}_{label:?}_ms").replace('"', "\"\"");
        write!(output, ",\"{name}\"")?;
    }
    writeln!(output)?;
    output.flush()?;
    capture
        .lock()
        .map_err(|_| io::Error::other("frame capture lock is poisoned"))?
        .main_detail = Some(MainScheduleCapture {
        output,
        phases: vec![Duration::ZERO; original.len()],
        previous: None,
        start: None,
    });
    let start_capture = Arc::clone(capture);
    app.add_systems(MainTimingStart, move || start_main(&start_capture));
    let mut instrumented = Vec::with_capacity(original.len() * 2 + 1);
    instrumented.push(MainTimingStart.intern());
    let last = original.len() - 1;
    for (index, label) in original.into_iter().enumerate() {
        let boundary_capture = Arc::clone(capture);
        instrumented.push(label);
        instrumented.push(MainTimingBoundary(index).intern());
        app.add_systems(MainTimingBoundary(index), move |world: &mut World| {
            let Ok(mut capture) = boundary_capture.lock() else {
                return;
            };
            let sample = capture.frame.saturating_sub(capture.warmup);
            let flush = sample.is_multiple_of(60) || capture.limit == Some(sample);
            let Some(detail) = capture.main_detail.as_mut() else {
                return;
            };
            let now = Instant::now();
            if let Some(previous) = detail.previous.replace(now) {
                detail.phases[index] = now - previous;
            }
            if index != last {
                return;
            }
            if sample == 0 {
                return;
            }
            let total = detail.start.map_or(Duration::ZERO, |start| now - start);
            let tick = world.resource::<SimWorld>().state.tick.get();
            if let Err(error) = detail.write_sample(sample, tick, total) {
                error!("Cannot write main schedule capture: {error}");
                world.write_message(AppExit::error());
            } else if flush && let Err(error) = detail.output.flush() {
                error!("Cannot flush main schedule capture: {error}");
                world.write_message(AppExit::error());
            }
        });
    }
    app.world_mut().resource_mut::<MainScheduleOrder>().labels = instrumented;
    Ok(())
}

fn ensure_distinct_output(normal: &Path, detailed: &Path) -> io::Result<()> {
    let same_path = match (normal.canonicalize(), detailed.canonicalize()) {
        (Ok(normal), Ok(detailed)) => normal == detailed,
        _ => false,
    };
    #[cfg(unix)]
    let same_path = {
        use std::os::unix::fs::MetadataExt;
        same_path
            || match (normal.metadata(), detailed.metadata()) {
                (Ok(normal), Ok(detailed)) => {
                    normal.dev() == detailed.dev() && normal.ino() == detailed.ino()
                }
                _ => false,
            }
    };
    if same_path {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "normal and main schedule captures require distinct files",
        ));
    }
    Ok(())
}

fn start_main(capture: &SharedFrameCapture) {
    let Ok(mut capture) = capture.lock() else {
        return;
    };
    let Some(detail) = capture.main_detail.as_mut() else {
        return;
    };
    let now = Instant::now();
    detail.start = Some(now);
    detail.previous = Some(now);
    detail.phases.fill(Duration::ZERO);
}

impl MainScheduleCapture {
    fn write_sample(&mut self, frame: u32, tick: u64, total: Duration) -> io::Result<()> {
        write!(
            self.output,
            "{frame},{tick},{:.4}",
            total.as_secs_f64() * 1000.0
        )?;
        for phase in &self.phases {
            write!(self.output, ",{:.4}", phase.as_secs_f64() * 1000.0)?;
        }
        writeln!(self.output)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::performance::FrameCapture;
    use openttdrs_core::GameState;
    use std::sync::Mutex;

    #[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
    struct CustomPhase;

    #[derive(Resource, Default)]
    struct Visits(Vec<usize>);

    #[test]
    fn timing_boundaries_preserve_custom_schedules_frames_and_ticks() {
        let folder =
            std::env::temp_dir().join(format!("openttdrs-main-timing-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("fixture folder");
        let normal_path = folder.join("normal.csv");
        let detail_path = folder.join("detail.csv");
        let mut app = App::new();
        app.world_mut()
            .resource_mut::<MainScheduleOrder>()
            .insert_after(Update, CustomPhase);
        let original = app.world().resource::<MainScheduleOrder>().labels.clone();
        app.init_resource::<Visits>();
        for (index, label) in original.iter().copied().enumerate() {
            app.add_systems(label, move |mut visits: ResMut<Visits>| {
                visits.0.push(index);
            });
        }
        let capture = Arc::new(Mutex::new(FrameCapture {
            output: BufWriter::new(File::create(&normal_path).expect("fixture output")),
            path: normal_path.clone(),
            main_detail: None,
            frame: 0,
            warmup: 2,
            limit: Some(3),
            previous: Instant::now(),
            start: Instant::now(),
            frame_time: Duration::ZERO,
            scale: None,
            pan: false,
            pan_origin: None,
        }));
        app.insert_resource(SimWorld {
            state: GameState::new(4, 4),
            loaded_file: false,
            ottdmap_extras: None,
        });
        let first_capture = Arc::clone(&capture);
        app.add_systems(First, move || {
            first_capture.lock().expect("fixture capture").frame += 1;
        });
        app.add_systems(Update, |mut sim: ResMut<SimWorld>| {
            sim.state.tick.advance();
        });
        install(&mut app, &detail_path, &capture).expect("install capture");
        let retained: Vec<_> = app
            .world()
            .resource::<MainScheduleOrder>()
            .labels
            .iter()
            .copied()
            .filter(|label| original.contains(label))
            .collect();
        assert_eq!(retained, original);
        for _ in 0..5 {
            app.update();
        }
        let expected: Vec<_> = (0..5).flat_map(|_| 0..original.len()).collect();
        assert_eq!(app.world().resource::<Visits>().0, expected);
        assert_eq!(app.world().resource::<SimWorld>().state.tick.get(), 5);
        let content = std::fs::read_to_string(&detail_path).expect("complete capture");
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("CustomPhase"));
        for (index, line) in lines[1..].iter().enumerate() {
            let fields: Vec<_> = line.split(',').collect();
            assert_eq!(fields[0].parse::<usize>().expect("frame"), index + 1);
            assert_eq!(fields[1].parse::<usize>().expect("tick"), index + 3);
            let total = fields[2].parse::<f64>().expect("main duration");
            let sum: f64 = fields[3..]
                .iter()
                .map(|field| field.parse::<f64>().expect("phase duration"))
                .sum();
            assert!((sum - total).abs() < 0.001);
        }
        let instrumented = app.world().resource::<MainScheduleOrder>().labels.clone();
        assert!(install(&mut app, &normal_path, &capture).is_err());
        #[cfg(unix)]
        {
            let alias = folder.join("normal-alias.csv");
            std::fs::hard_link(&normal_path, &alias).expect("fixture alias");
            assert!(install(&mut app, &alias, &capture).is_err());
        }
        let invalid_path = normal_path.join("cannot-be-created.csv");
        assert!(install(&mut app, &invalid_path, &capture).is_err());
        assert_eq!(
            app.world().resource::<MainScheduleOrder>().labels,
            instrumented
        );
        assert_eq!(
            std::fs::read_to_string(detail_path).expect("capture preserved"),
            content
        );
    }
}
