//! Puente `SimEvent` (core) → SFX/FX del cliente.

use bevy::prelude::*;

use openttdrs_core::prelude::*;
use openttdrs_core::{
    ConstructionKind, SoundId, VehicleRunningPhase, VehicleSoundEvent, VehicleSoundOverride,
    play_newgrf_sound, resolve_vehicle_sound_callback,
};

use crate::audio::PlayWorldSfx;
use crate::bevy_app::{FixedUpdateSet, UpdateSet};
use crate::render::BubbleSpawnQueue;
use crate::render::effect_fx::FxSpawnQueue;
use crate::state::{ClientScreen, SimWorld};
use crate::ui::SimHudControls;

/// Eventos drenados del último tick de simulación.
#[derive(Resource, Default)]
pub(crate) struct PendingSimEvents(pub Vec<SimEvent>);

pub(crate) struct SimEventsPlugin;

impl Plugin for SimEventsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingSimEvents>()
            .init_resource::<FxSpawnQueue>()
            .init_resource::<BubbleSpawnQueue>()
            // Este puente conserva los eventos de render aunque el flujo sea
            // una captura silenciosa y `WorldSfxPlugin` no esté instalado.
            // En una partida normal el mismo registro lo consume el mixer.
            .add_message::<PlayWorldSfx>()
            .add_systems(
                OnEnter(ClientScreen::InGame),
                discard_bootstrap_sim_events_on_enter,
            )
            .add_systems(
                FixedUpdate,
                drain_sim_events_from_core
                    .in_set(FixedUpdateSet::Events)
                    .run_if(in_state(ClientScreen::InGame)),
            )
            .add_systems(
                Update,
                dispatch_sim_events
                    .in_set(UpdateSet::Sim)
                    .run_if(in_state(ClientScreen::InGame)),
            );
    }
}

fn discard_bootstrap_sim_events_on_enter(
    mut sim: ResMut<SimWorld>,
    mut pending: ResMut<PendingSimEvents>,
) {
    sim.state.runtime.pending_sim_events.discard_all();
    pending.0.clear();
}

fn drain_sim_events_from_core(mut sim: ResMut<SimWorld>, mut pending: ResMut<PendingSimEvents>) {
    pending
        .0
        .extend(sim.state.runtime.pending_sim_events.drain());
}

#[must_use]
const fn aircraft_takeoff_sound(engine_id: u16) -> SoundId {
    if openttdrs_core::aircraft_is_helicopter(engine_id) {
        SoundId::TakeoffHelicopter
    } else if openttdrs_core::aircraft_is_jet(engine_id) {
        SoundId::TakeoffJet
    } else {
        SoundId::TakeoffPropeller
    }
}

#[must_use]
const fn aircraft_landing_sound(engine_id: u16) -> SoundId {
    if openttdrs_core::aircraft_is_helicopter(engine_id) {
        SoundId::TakeoffHelicopter
    } else {
        SoundId::SkidPlane
    }
}

/// Nivel de detalle para el registro del bus de eventos. Todo `SimEvent` se
/// conserva en la traza: los eventos frecuentes se bajan a `debug` para que
/// una partida normal no produzca miles de líneas por minuto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimEventLogLevel {
    Info,
    Debug,
}

const fn sim_event_log_level(event: &SimEvent) -> SimEventLogLevel {
    match event {
        // Ingresos, pulsos de motor y ambiente son parte del ciclo normal.
        SimEvent::Income { .. }
        | SimEvent::VehicleDepart { .. }
        | SimEvent::VehicleRunning { .. }
        | SimEvent::VehicleTunnel { .. }
        | SimEvent::VehicleVisualEffect { .. }
        | SimEvent::VehicleLoadUnload { .. }
        | SimEvent::LevelCrossing { .. }
        | SimEvent::Bubble { .. }
        | SimEvent::AircraftTakeoff { .. }
        | SimEvent::AircraftLanding { .. } => SimEventLogLevel::Debug,
        // Obras, avisos, cambios de autoridad, subvenciones y fallos son
        // decisiones o resultados relevantes para el jugador.
        SimEvent::Construction { .. }
        | SimEvent::Demolition { .. }
        | SimEvent::Breakdown { .. }
        | SimEvent::Disaster { .. }
        | SimEvent::TownRatingChanged { .. }
        | SimEvent::SubsidyCreated { .. }
        | SimEvent::SubsidyAwarded { .. }
        | SimEvent::NewsTicker
        | SimEvent::NewsApplause
        | SimEvent::NewsChime
        | SimEvent::LoanInterestPaid { .. }
        | SimEvent::BankruptcyWarning
        | SimEvent::TrainCollision { .. }
        | SimEvent::GameOver { .. }
        | SimEvent::AircraftCrash { .. }
        | SimEvent::RoadVehCrash { .. }
        | SimEvent::VehicleFlooded { .. } => SimEventLogLevel::Info,
    }
}

fn log_sim_event(event: &SimEvent) {
    match sim_event_log_level(event) {
        SimEventLogLevel::Info => info!("evento: {event:?}"),
        SimEventLogLevel::Debug => debug!("evento: {event:?}"),
    }
}

// Agrupa el puente core→SFX: cada evento necesita id, callback, fallback,
// posición y los parámetros del mixer para conservar la semántica vanilla.
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_vehicle_event_sound(
    sim: &mut SimWorld,
    sfx: &mut MessageWriter<PlayWorldSfx>,
    vehicle_id: u32,
    event: VehicleSoundEvent,
    default_sound: SoundId,
    at: TileCoord,
    volume: f32,
    priority: u8,
) {
    play_vehicle_event_sound_with_default(
        sim,
        sfx,
        vehicle_id,
        event,
        Some(default_sound),
        at,
        volume,
        priority,
    );
}

/// Variante para eventos cuyo comportamiento vanilla no reproduce ningún
/// sonido cuando el callback no está implementado (por ejemplo, humo/chispas).
#[allow(clippy::too_many_arguments)]
pub(crate) fn play_vehicle_event_sound_with_default(
    sim: &mut SimWorld,
    sfx: &mut MessageWriter<PlayWorldSfx>,
    vehicle_id: u32,
    event: VehicleSoundEvent,
    default_sound: Option<SoundId>,
    at: TileCoord,
    volume: f32,
    priority: u8,
) {
    match resolve_vehicle_sound_callback(&mut sim.state, vehicle_id, event) {
        VehicleSoundOverride::Default => {
            if let Some(sound) = default_sound {
                sfx.write(PlayWorldSfx::new(sound, at, volume).with_priority(priority));
            }
        }
        VehicleSoundOverride::Base(sound) => {
            sfx.write(PlayWorldSfx::new(sound, at, volume).with_priority(priority));
        }
        VehicleSoundOverride::Newgrf { grfid, local_id } => {
            // El helper ya validó la entrada y su PCM; el mixer drena la cola
            // en el mismo frame que este puente procesa el evento.
            let _ = play_newgrf_sound(&mut sim.state, grfid, local_id);
        }
        VehicleSoundOverride::Suppressed => {}
    }
}

fn dispatch_sim_events(
    mut pending: ResMut<PendingSimEvents>,
    mut sim: ResMut<SimWorld>,
    hud: Res<SimHudControls>,
    mut sfx: MessageWriter<PlayWorldSfx>,
    mut fx: ResMut<FxSpawnQueue>,
    mut bubbles: ResMut<BubbleSpawnQueue>,
) {
    for event in pending.0.drain(..) {
        log_sim_event(&event);
        match event {
            SimEvent::Income { at, .. } => {
                if hud.sound_confirm {
                    sfx.write(PlayWorldSfx::new(SoundId::CashTill, at, 1.0).with_priority(80));
                }
            }
            SimEvent::Construction { kind, at } => {
                // OpenTTD reproduce el SFX de construcción con `sound.confirm`.
                if hud.sound_confirm {
                    let sound = match kind {
                        ConstructionKind::Rail => SoundId::ConstructionRail,
                        ConstructionKind::Bridge => SoundId::ConstructionBridge,
                        ConstructionKind::Water => SoundId::ConstructionWater,
                        ConstructionKind::Road | ConstructionKind::Other => {
                            SoundId::ConstructionOther
                        }
                    };
                    sfx.write(PlayWorldSfx::new(sound, at, 0.85).with_priority(70));
                }
            }
            SimEvent::Demolition { at } => {
                if hud.sound_confirm {
                    sfx.write(PlayWorldSfx::new(SoundId::Explosion, at, 0.5).with_priority(90));
                }
                fx.push_explosion(at);
            }
            SimEvent::VehicleDepart {
                vehicle_id,
                at,
                kind,
            } => {
                if hud.sound_vehicle {
                    play_vehicle_event_sound(
                        &mut sim,
                        &mut sfx,
                        vehicle_id,
                        VehicleSoundEvent::Start,
                        SoundId::departure_for_kind(kind),
                        at,
                        0.9,
                        72,
                    );
                }
            }
            SimEvent::VehicleRunning {
                vehicle_id,
                at,
                kind,
                phase,
            } => {
                if !hud.sound_vehicle {
                    continue;
                }
                let (volume, priority) = match phase {
                    VehicleRunningPhase::Running => (0.22, 8),
                    VehicleRunningPhase::Running16 => (0.18, 6),
                    VehicleRunningPhase::Stopped16 => (0.12, 4),
                };
                let event = match phase {
                    VehicleRunningPhase::Running => VehicleSoundEvent::Running,
                    VehicleRunningPhase::Running16 => VehicleSoundEvent::Running16,
                    VehicleRunningPhase::Stopped16 => VehicleSoundEvent::Stopped16,
                };
                play_vehicle_event_sound(
                    &mut sim,
                    &mut sfx,
                    vehicle_id,
                    event,
                    SoundId::running_for_kind(kind),
                    at,
                    volume,
                    priority,
                );
            }
            SimEvent::VehicleTunnel {
                vehicle_id,
                at,
                kind,
            } => {
                if hud.sound_vehicle && kind == VehicleKind::Train {
                    let default = sim
                        .state
                        .vehicles
                        .iter()
                        .find(|vehicle| vehicle.id == vehicle_id)
                        .and_then(|vehicle| vehicle.engine_id)
                        .unwrap_or_else(|| openttdrs_core::default_engine_id(VehicleKind::Train));
                    let default = (openttdrs_core::train_smoke_kind(default)
                        == openttdrs_core::TrainSmokeKind::Steam)
                        .then_some(SoundId::TrainThroughTunnel);
                    play_vehicle_event_sound_with_default(
                        &mut sim,
                        &mut sfx,
                        vehicle_id,
                        VehicleSoundEvent::Tunnel,
                        default,
                        at,
                        0.85,
                        74,
                    );
                }
            }
            SimEvent::VehicleVisualEffect { .. } => {
                // El renderer de humo resuelve el callback en el mismo frame
                // en que crea el efecto, por lo que no se reproduce dos veces
                // aquí si el evento queda pendiente de un tick anterior.
            }
            SimEvent::VehicleLoadUnload {
                vehicle_id,
                at,
                kind: _,
            } => {
                if hud.sound_vehicle {
                    play_vehicle_event_sound(
                        &mut sim,
                        &mut sfx,
                        vehicle_id,
                        VehicleSoundEvent::LoadUnload,
                        SoundId::CashTill,
                        at,
                        0.8,
                        70,
                    );
                }
            }
            SimEvent::LevelCrossing { at } => {
                if hud.sound_vehicle {
                    sfx.write(PlayWorldSfx::new(SoundId::LevelCrossing, at, 0.8).with_priority(75));
                }
            }
            SimEvent::Breakdown {
                vehicle_id,
                at,
                kind,
            } => {
                if hud.sound_vehicle {
                    play_vehicle_event_sound(
                        &mut sim,
                        &mut sfx,
                        vehicle_id,
                        VehicleSoundEvent::Breakdown,
                        SoundId::breakdown_for_kind(kind),
                        at,
                        0.7,
                        85,
                    );
                }
                fx.push_breakdown(at);
            }
            SimEvent::Bubble { at, direction } => {
                bubbles.push(at, direction);
                if hud.sound_ambient {
                    sfx.write(
                        PlayWorldSfx::new(SoundId::BubbleGenerator, at, 0.55).with_priority(12),
                    );
                }
            }
            SimEvent::Disaster { at, .. } => {
                if hud.sound_disaster {
                    sfx.write(PlayWorldSfx::new(SoundId::Explosion, at, 1.0).with_priority(120));
                }
                fx.push_explosion(at);
            }
            SimEvent::TrainCollision { at, .. } => {
                if hud.sound_disaster || hud.sound_vehicle {
                    sfx.write(
                        PlayWorldSfx::new(SoundId::TrainCollision, at, 1.0).with_priority(130),
                    );
                }
                fx.push_explosion(at);
            }
            SimEvent::NewsTicker => {
                if hud.sound_confirm {
                    sfx.write(
                        PlayWorldSfx::new(
                            SoundId::NewsTicker,
                            openttdrs_core::TileCoord::new(0, 0),
                            0.6,
                        )
                        .with_priority(60),
                    );
                }
            }
            SimEvent::NewsApplause => {
                if hud.sound_confirm {
                    sfx.write(
                        PlayWorldSfx::new(
                            SoundId::Applause,
                            openttdrs_core::TileCoord::new(0, 0),
                            0.7,
                        )
                        .with_priority(60),
                    );
                }
            }
            SimEvent::NewsChime => {
                if hud.sound_confirm {
                    sfx.write(
                        PlayWorldSfx::new(
                            SoundId::NewEngine,
                            openttdrs_core::TileCoord::new(0, 0),
                            0.6,
                        )
                        .with_priority(60),
                    );
                }
            }
            SimEvent::LoanInterestPaid { .. }
            | SimEvent::BankruptcyWarning
            | SimEvent::GameOver { .. } => {}
            SimEvent::AircraftTakeoff {
                vehicle_id,
                at,
                engine_id,
            } => {
                if hud.sound_vehicle {
                    play_vehicle_event_sound(
                        &mut sim,
                        &mut sfx,
                        vehicle_id,
                        VehicleSoundEvent::Start,
                        aircraft_takeoff_sound(engine_id),
                        at,
                        0.85,
                        78,
                    );
                }
            }
            SimEvent::AircraftLanding {
                vehicle_id,
                at,
                engine_id,
            } => {
                if hud.sound_vehicle {
                    play_vehicle_event_sound(
                        &mut sim,
                        &mut sfx,
                        vehicle_id,
                        VehicleSoundEvent::Touchdown,
                        aircraft_landing_sound(engine_id),
                        at,
                        0.8,
                        78,
                    );
                }
            }
            SimEvent::AircraftCrash { at, .. } => {
                if hud.sound_disaster || hud.sound_vehicle {
                    sfx.write(PlayWorldSfx::new(SoundId::Explosion, at, 1.0).with_priority(130));
                }
                fx.push_explosion(at);
            }
            SimEvent::RoadVehCrash { at, .. } => {
                if hud.sound_disaster || hud.sound_vehicle {
                    sfx.write(PlayWorldSfx::new(SoundId::Explosion, at, 1.0).with_priority(130));
                }
                fx.push_explosion(at);
            }
            SimEvent::VehicleFlooded { at, .. } => {
                if hud.sound_disaster || hud.sound_vehicle {
                    sfx.write(PlayWorldSfx::new(SoundId::Explosion, at, 1.0).with_priority(125));
                }
                fx.push_explosion(at);
            }
            SimEvent::TownRatingChanged { .. } => {}
            SimEvent::SubsidyCreated { station_pos, .. } => {
                if hud.sound_confirm {
                    sfx.write(
                        PlayWorldSfx::new(SoundId::NewsTicker, station_pos, 0.65).with_priority(70),
                    );
                }
            }
            SimEvent::SubsidyAwarded { .. } => {
                if hud.sound_confirm {
                    sfx.write(
                        PlayWorldSfx::new(
                            SoundId::Applause,
                            openttdrs_core::TileCoord::new(0, 0),
                            0.7,
                        )
                        .with_priority(70),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod aircraft_sound_tests {
    use super::*;

    #[test]
    fn selects_helicopter_propeller_and_jet_sounds_by_engine() {
        assert_eq!(
            aircraft_takeoff_sound(openttdrs_core::ENGINE_AIRCRAFT_TRICARIO),
            SoundId::TakeoffHelicopter
        );
        assert_eq!(
            aircraft_takeoff_sound(openttdrs_core::ENGINE_AIRCRAFT_DAKOTA),
            SoundId::TakeoffPropeller
        );
        assert_eq!(
            aircraft_takeoff_sound(openttdrs_core::ENGINE_AIRCRAFT_FOKKER),
            SoundId::TakeoffJet
        );
        assert_eq!(
            aircraft_landing_sound(openttdrs_core::ENGINE_AIRCRAFT_TRICARIO),
            SoundId::TakeoffHelicopter
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn income_event_drains_from_game_state() {
        let mut sim = SimWorld {
            state: GameState::new(4, 4),
            ..Default::default()
        };
        sim.state.runtime.pending_sim_events.push(SimEvent::Income {
            amount: 100,
            at: TileCoord::new(1, 1),
        });
        let drained = sim.state.runtime.pending_sim_events.drain();
        assert_eq!(drained.len(), 1);
    }

    #[test]
    fn pending_bridge_holds_events_until_dispatch() {
        // Latencia FixedUpdate → Update: drain llena Pending; dispatch vacía.
        let mut pending = PendingSimEvents::default();
        pending.0.push(SimEvent::Income {
            amount: 1,
            at: TileCoord::new(0, 0),
        });
        assert_eq!(pending.0.len(), 1);
        let drained = std::mem::take(&mut pending.0);
        assert_eq!(drained.len(), 1);
        assert!(pending.0.is_empty());
    }

    #[test]
    fn event_log_levels_keep_periodic_activity_out_of_normal_output() {
        assert_eq!(
            sim_event_log_level(&SimEvent::Income {
                amount: 10,
                at: TileCoord::new(1, 2),
            }),
            SimEventLogLevel::Debug
        );
        assert_eq!(
            sim_event_log_level(&SimEvent::VehicleRunning {
                vehicle_id: 1,
                at: TileCoord::new(1, 2),
                kind: openttdrs_core::VehicleKind::Bus,
                phase: VehicleRunningPhase::Running,
            }),
            SimEventLogLevel::Debug
        );
        assert_eq!(
            sim_event_log_level(&SimEvent::Construction {
                kind: ConstructionKind::Road,
                at: TileCoord::new(1, 2),
            }),
            SimEventLogLevel::Info
        );
        assert_eq!(
            sim_event_log_level(&SimEvent::TownRatingChanged {
                town_id: 7,
                delta: 20,
            }),
            SimEventLogLevel::Info
        );
    }
}
