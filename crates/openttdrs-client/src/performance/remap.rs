//! Opt-in timing of the remap system's own deferred command queue.
//! State stays outside ECS; the two queued markers create no entities/resources.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use bevy::prelude::*;

type Output = Arc<Mutex<BufWriter<File>>>;
static OUTPUT: OnceLock<Option<Output>> = OnceLock::new();

#[derive(Default)]
pub(crate) struct RemapCounts {
    pub(crate) incremental: bool,
    pub(crate) refreshed_chunks: usize,
    pub(crate) added_chunks: usize,
    pub(crate) removed_chunks: usize,
    pub(crate) despawned_visuals: usize,
}

pub(crate) struct RemapTrace {
    output: Output,
    apply_start: Arc<Mutex<Option<Instant>>>,
    prepare_start: Instant,
    tick: u64,
    full: bool,
    pub(crate) counts: RemapCounts,
}

fn output_from_env() -> Option<Output> {
    let path = std::env::var_os("OPENTTDRS_REMAP_TRACE_OUT")?;
    let result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|file| {
            let mut output = BufWriter::new(file);
            writeln!(output, "tick,full,incremental,refreshed_chunks,added_chunks,removed_chunks,despawned_visuals,prepare_ms,apply_commands_ms")?;
            output.flush()?;
            Ok(Arc::new(Mutex::new(output)))
        });
    match result {
        Ok(output) => Some(output),
        Err(error) => {
            error!("Cannot create remap command trace: {error}");
            None
        }
    }
}

impl RemapTrace {
    pub(crate) fn start(commands: &mut Commands, tick: u64, full: bool) -> Option<Self> {
        let output = OUTPUT.get_or_init(output_from_env).as_ref()?.clone();
        Some(Self::with_output(commands, tick, full, output))
    }

    fn with_output(commands: &mut Commands, tick: u64, full: bool, output: Output) -> Self {
        let apply_start = Arc::new(Mutex::new(None));
        let marker = Arc::clone(&apply_start);
        commands.queue(move |_: &mut World| {
            if let Ok(mut marker) = marker.lock() {
                *marker = Some(Instant::now());
            }
        });
        Self {
            output,
            apply_start,
            prepare_start: Instant::now(),
            tick,
            full,
            counts: RemapCounts::default(),
        }
    }

    pub(crate) fn finish(self, commands: &mut Commands) {
        let prepare_ms = self.prepare_start.elapsed().as_secs_f64() * 1000.0;
        commands.queue(move |_: &mut World| {
            let apply_ms = self
                .apply_start
                .lock()
                .ok()
                .and_then(|start| *start)
                .map(|start| start.elapsed().as_secs_f64() * 1000.0);
            let Some(apply_ms) = apply_ms else {
                error!("Remap command trace has no initial marker");
                return;
            };
            let Ok(mut output) = self.output.lock() else {
                error!("Remap command trace lock is poisoned");
                return;
            };
            let counts = self.counts;
            let result = writeln!(
                output,
                "{},{},{},{},{},{},{},{:.6},{:.6}",
                self.tick,
                self.full,
                counts.incremental,
                counts.refreshed_chunks,
                counts.added_chunks,
                counts.removed_chunks,
                counts.despawned_visuals,
                prepare_ms,
                apply_ms
            )
            .and_then(|()| output.flush());
            if let Err(error) = result {
                error!("Cannot write remap command trace: {error}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;

    #[test]
    fn trace_markers_wrap_commands_without_registering_ecs_state()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("remap.csv");
        let output = Arc::new(Mutex::new(BufWriter::new(File::create(&path)?)));
        let mut world = World::new();
        let components = world.components().len();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let mut trace = RemapTrace::with_output(&mut commands, 73, false, output);
        assert_eq!(world.components().len(), components);
        let entity = commands.spawn_empty().id();
        trace.counts.despawned_visuals = 1;
        commands.entity(entity).despawn();
        trace.finish(&mut commands);
        assert_eq!(std::fs::read_to_string(&path)?, "");
        queue.apply(&mut world);
        assert!(!world.entities().contains(entity));
        assert_eq!(world.components().len(), components);
        let row = std::fs::read_to_string(path)?;
        let fields: Vec<_> = row.trim().split(',').collect();
        assert_eq!(fields.len(), 9);
        assert_eq!(&fields[..7], ["73", "false", "false", "0", "0", "0", "1"]);
        assert!(fields[7].parse::<f64>()?.is_finite());
        assert!(fields[8].parse::<f64>()?.is_finite());
        Ok(())
    }
}
