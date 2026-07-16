//! Traza de render opt-in (`OPENTTDRS_RENDER_TRACE=/ruta/salida.csv`).
//!
//! Registra por frame y por vehículo la pose lógica (estado de sim), la pose
//! extrapolada (lo que se dibuja), `tick_alpha` y la dirección del sprite.
//! Sirve para separar problemas de simulación de problemas de
//! interpolación/render: la columna lógica solo cambia a 5 Hz, la extrapolada
//! debería avanzar suave a FPS de render.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Mutex;

use bevy::prelude::*;
use openttdrs_core::{extrapolate_vehicle_pose, vehicle_render_direction_at, vehicle_subtile_at};

use crate::bevy_app::UpdateSet;
use crate::simulation::SimClock;
use crate::state::SimWorld;

/// Variable de entorno con la ruta del CSV de salida.
pub(crate) const RENDER_TRACE_ENV: &str = "OPENTTDRS_RENDER_TRACE";

const CSV_HEADER: &str = "frame,tick,tick_alpha,vehicle,logical_tile_x,logical_tile_y,\
logical_progress,logical_dir,extrap_tile_x,extrap_tile_y,extrap_progress,sprite_dir,\
logical_subtile_x,logical_subtile_y,extrap_subtile_x,extrap_subtile_y,\
logical_world_x,logical_world_y,extrap_world_x,extrap_world_y";

#[derive(Resource, Default)]
pub(crate) struct RenderTrace {
    writer: Option<Mutex<BufWriter<File>>>,
}

impl RenderTrace {
    fn from_env() -> Self {
        let Ok(path) = std::env::var(RENDER_TRACE_ENV) else {
            return Self::default();
        };
        if path.trim().is_empty() {
            return Self::default();
        }
        match File::create(&path) {
            Ok(file) => {
                let mut writer = BufWriter::new(file);
                let _ = writeln!(writer, "{CSV_HEADER}");
                info!("traza de render activa: {path}");
                Self {
                    writer: Some(Mutex::new(writer)),
                }
            }
            Err(e) => {
                warn!("no se pudo crear {RENDER_TRACE_ENV}={path}: {e}");
                Self::default()
            }
        }
    }
}

pub(crate) struct RenderTracePlugin;

impl Plugin for RenderTracePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderTrace::from_env()).add_systems(
            Update,
            record_render_trace
                .in_set(UpdateSet::Visuals)
                .run_if(|trace: Res<RenderTrace>| trace.writer.is_some()),
        );
    }
}

fn record_render_trace(
    trace: Res<RenderTrace>,
    sim: Res<SimWorld>,
    sim_clock: Res<SimClock>,
    mut frame: Local<u64>,
) {
    let Some(writer) = &trace.writer else {
        return;
    };
    let Ok(mut writer) = writer.lock() else {
        return;
    };
    let tick = sim.state.tick.get();
    let alpha = sim_clock.tick_alpha;
    for v in &sim.state.vehicles {
        let pose = extrapolate_vehicle_pose(v, alpha);
        let sprite_dir = vehicle_render_direction_at(v, pose);
        let logical_pose = extrapolate_vehicle_pose(v, 0.0);
        let logical_world = crate::render::vehicle_sprite_pos_at(v, &sim.state.map, logical_pose);
        let extrap_world = crate::render::vehicle_sprite_pos_at(v, &sim.state.map, pose);
        let (logical_sub_x, logical_sub_y) = vehicle_subtile_at(v, logical_pose);
        let (extrap_sub_x, extrap_sub_y) = vehicle_subtile_at(v, pose);
        let _ = writeln!(
            writer,
            "{},{},{:.4},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.2},{:.2},{:.2},{:.2}",
            *frame,
            tick,
            alpha,
            v.id,
            v.pos.x,
            v.pos.y,
            v.progress,
            v.direction,
            pose.pos.x,
            pose.pos.y,
            pose.progress,
            sprite_dir,
            logical_sub_x,
            logical_sub_y,
            extrap_sub_x,
            extrap_sub_y,
            logical_world.x,
            logical_world.y,
            extrap_world.x,
            extrap_world.y,
        );
    }
    *frame += 1;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::prelude::*;

    #[test]
    fn disabled_without_env_and_records_rows_when_writer_present() {
        // Sin env: recurso inerte.
        let trace = RenderTrace::default();
        assert!(trace.writer.is_none());

        // Con writer: una fila por vehículo y frame.
        let dir = std::env::temp_dir().join("openttdrs_render_trace_test.csv");
        let file = File::create(&dir).unwrap();
        let mut writer = BufWriter::new(file);
        writeln!(writer, "{CSV_HEADER}").unwrap();

        let mut sim = SimWorld {
            state: openttdrs_core::GameState::new(8, 8),
            loaded_file: false,
            ottdmap_extras: None,
        };
        let mut v = Vehicle::new(
            7,
            VehicleKind::Truck,
            TileCoord::new(1, 1),
            TileCoord::new(2, 1),
        );
        v.path = std::collections::VecDeque::from([TileCoord::new(2, 1)]);
        v.set_cruise_speed();
        sim.state.vehicles.push(v);

        let mut world = World::new();
        world.insert_resource(RenderTrace {
            writer: Some(Mutex::new(writer)),
        });
        world.insert_resource(sim);
        world.insert_resource(SimClock { tick_alpha: 0.5 });
        world.run_system_once(record_render_trace).unwrap();
        world.run_system_once(record_render_trace).unwrap();

        // Volcar y verificar: cabecera + 2 filas (una por frame y vehículo).
        drop(world);
        let contents = std::fs::read_to_string(&dir).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 3, "cabecera + 2 filas: {contents}");
        assert!(lines[0].starts_with("frame,tick,tick_alpha"));
        assert!(lines[0].contains("logical_subtile_x"));
        // Con tick_alpha=0.5 la pose extrapolada difiere de la lógica.
        let cols: Vec<&str> = lines[1].split(',').collect();
        assert_eq!(cols[3], "7", "id del vehículo");
        assert_ne!(cols[6], cols[10], "progress extrapolado ≠ lógico");
        assert_ne!(cols[12], cols[14], "subtile_x extrapolado ≠ lógico");
        let _ = std::fs::remove_file(&dir);
    }
}
