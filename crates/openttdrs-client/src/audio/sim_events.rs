//! Puente `SimEvent` (core) → SFX/FX del cliente.

use bevy::prelude::*;

use openttdrs_core::{ConstructionKind, SimEvent, SoundId};

use crate::audio::PlayWorldSfx;
use crate::bevy_app::UpdateSet;
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
            .add_systems(
                OnEnter(ClientScreen::InGame),
                discard_bootstrap_sim_events_on_enter,
            )
            .add_systems(
                FixedUpdate,
                drain_sim_events_from_core.run_if(in_state(ClientScreen::InGame)),
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
    sim.state.pending_sim_events.discard_all();
    pending.0.clear();
}

fn drain_sim_events_from_core(mut sim: ResMut<SimWorld>, mut pending: ResMut<PendingSimEvents>) {
    pending.0.extend(sim.state.pending_sim_events.drain());
}

fn dispatch_sim_events(
    mut pending: ResMut<PendingSimEvents>,
    hud: Res<SimHudControls>,
    mut sfx: MessageWriter<PlayWorldSfx>,
    mut fx: ResMut<FxSpawnQueue>,
) {
    for event in pending.0.drain(..) {
        match event {
            SimEvent::Income { at, .. } => {
                if hud.sound_confirm {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::CashTill,
                        at,
                        volume: 1.0,
                    });
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
                    sfx.write(PlayWorldSfx {
                        sound,
                        at,
                        volume: 0.85,
                    });
                }
            }
            SimEvent::Demolition { at } => {
                if hud.sound_confirm {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::Explosion,
                        at,
                        volume: 0.5,
                    });
                }
                fx.push_explosion(at);
            }
            SimEvent::VehicleDepart { at, .. } => {
                if hud.sound_vehicle {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::DepartureTrain,
                        at,
                        volume: 0.9,
                    });
                }
            }
            SimEvent::LevelCrossing { at } => {
                if hud.sound_vehicle {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::LevelCrossing,
                        at,
                        volume: 0.8,
                    });
                }
            }
            SimEvent::Breakdown { at, .. } => {
                if hud.sound_vehicle {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::Beep,
                        at,
                        volume: 0.7,
                    });
                }
                fx.push_breakdown(at);
            }
            SimEvent::Disaster { at, .. } => {
                if hud.sound_disaster {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::Explosion,
                        at,
                        volume: 1.0,
                    });
                }
                fx.push_explosion(at);
            }
            SimEvent::NewsTicker => {
                if hud.sound_confirm {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::NewsTicker,
                        at: openttdrs_core::TileCoord::new(0, 0),
                        volume: 0.6,
                    });
                }
            }
            SimEvent::NewsApplause => {
                if hud.sound_confirm {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::Applause,
                        at: openttdrs_core::TileCoord::new(0, 0),
                        volume: 0.7,
                    });
                }
            }
            SimEvent::NewsChime => {
                if hud.sound_confirm {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::NewEngine,
                        at: openttdrs_core::TileCoord::new(0, 0),
                        volume: 0.6,
                    });
                }
            }
            SimEvent::LoanInterestPaid { .. }
            | SimEvent::BankruptcyWarning
            | SimEvent::GameOver { .. } => {}
            SimEvent::AircraftTakeoff { at, .. } => {
                if hud.sound_vehicle {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::TakeoffHelicopter,
                        at,
                        volume: 0.85,
                    });
                }
            }
            SimEvent::AircraftLanding { at, .. } => {
                if hud.sound_vehicle {
                    sfx.write(PlayWorldSfx {
                        sound: SoundId::SkidPlane,
                        at,
                        volume: 0.8,
                    });
                }
            }
            SimEvent::TownRatingChanged { .. }
            | SimEvent::SubsidyCreated { .. }
            | SimEvent::SubsidyAwarded { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openttdrs_core::{GameState, TileCoord};

    #[test]
    fn income_event_drains_from_game_state() {
        let mut sim = SimWorld {
            state: GameState::new(4, 4),
            ..Default::default()
        };
        sim.state.pending_sim_events.push(SimEvent::Income {
            amount: 100,
            at: TileCoord::new(1, 1),
        });
        let drained = sim.state.pending_sim_events.drain();
        assert_eq!(drained.len(), 1);
    }
}
