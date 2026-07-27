//! Puente `SimEvent` (core) → SFX/FX del cliente.

use bevy::prelude::*;

use openttdrs_core::prelude::*;
use openttdrs_core::{ConstructionKind, SoundId, VehicleRunningPhase};

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

fn dispatch_sim_events(
    mut pending: ResMut<PendingSimEvents>,
    hud: Res<SimHudControls>,
    mut sfx: MessageWriter<PlayWorldSfx>,
    mut fx: ResMut<FxSpawnQueue>,
    mut bubbles: ResMut<BubbleSpawnQueue>,
) {
    for event in pending.0.drain(..) {
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
            SimEvent::VehicleDepart { at, kind, .. } => {
                if hud.sound_vehicle {
                    sfx.write(
                        PlayWorldSfx::new(SoundId::departure_for_kind(kind), at, 0.9)
                            .with_priority(72),
                    );
                }
            }
            SimEvent::VehicleRunning {
                at, kind, phase, ..
            } => {
                if !hud.sound_vehicle {
                    continue;
                }
                let (volume, priority) = match phase {
                    VehicleRunningPhase::Running => (0.22, 8),
                    VehicleRunningPhase::Running16 => (0.18, 6),
                    VehicleRunningPhase::Stopped16 => (0.12, 4),
                };
                sfx.write(
                    PlayWorldSfx::new(SoundId::running_for_kind(kind), at, volume)
                        .with_priority(priority),
                );
            }
            SimEvent::LevelCrossing { at } => {
                if hud.sound_vehicle {
                    sfx.write(PlayWorldSfx::new(SoundId::LevelCrossing, at, 0.8).with_priority(75));
                }
            }
            SimEvent::Breakdown { at, kind, .. } => {
                if hud.sound_vehicle {
                    sfx.write(
                        PlayWorldSfx::new(SoundId::breakdown_for_kind(kind), at, 0.7)
                            .with_priority(85),
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
            SimEvent::AircraftTakeoff { at, engine_id, .. } => {
                if hud.sound_vehicle {
                    sfx.write(
                        PlayWorldSfx::new(aircraft_takeoff_sound(engine_id), at, 0.85)
                            .with_priority(78),
                    );
                }
            }
            SimEvent::AircraftLanding { at, engine_id, .. } => {
                if hud.sound_vehicle {
                    sfx.write(
                        PlayWorldSfx::new(aircraft_landing_sound(engine_id), at, 0.8)
                            .with_priority(78),
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
        let drained: Vec<_> = pending.0.drain(..).collect();
        assert_eq!(drained.len(), 1);
        assert!(pending.0.is_empty());
    }
}
