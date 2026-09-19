use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use openttdrs_core::CommandError;

use super::HudBuildFeedback;
use crate::i18n::{Locale, localized_text};
use crate::state::SimWorld;
use crate::state::ingame_lifecycle::InGameUi;
use crate::ui::command_error_text::command_error_message;
use crate::ui::floating_window::window_text_font;
use crate::ui::font::UiFontRole;

const HUD_FEEDBACK_DISPLAY_SECS: f32 = 5.0;
const HUD_FEEDBACK_BOTTOM: f32 = 42.0;
const HUD_FEEDBACK_Z: i32 = 2200;
const FEEDBACK_BG: Color = Color::srgba(0.08, 0.075, 0.055, 0.96);
const FEEDBACK_BORDER: Color = Color::srgb(0.72, 0.64, 0.39);
const FEEDBACK_TEXT: Color = Color::srgb(0.98, 0.91, 0.67);

/// Raíz del aviso temporal que se muestra incluso si el HUD técnico está
/// oculto. No recibe foco ni clics: sólo hace visible el contenido de
/// [`HudBuildFeedback`] durante su vencimiento.
#[derive(Component)]
pub(crate) struct HudFeedbackToastRoot;

#[derive(Component)]
pub(crate) struct HudFeedbackToastText;

/// Inserta un aviso temporal compartido por acciones de HUD.
///
/// Los errores pueden pedir el pitido suave existente; las confirmaciones no
/// producen sonido para no competir con la partida.
pub(crate) fn push_hud_feedback(
    feedback: &mut HudBuildFeedback,
    message: String,
    elapsed_secs: f32,
    is_error: bool,
) {
    feedback.message = Some(message);
    feedback.expires_at_secs = elapsed_secs + HUD_FEEDBACK_DISPLAY_SECS;
    feedback.pending_soft_ping = is_error;
}

/// Muestra un mensaje temporal en el HUD y encola pitido suave.
pub(crate) fn push_build_command_error(
    feedback: &mut HudBuildFeedback,
    err: CommandError,
    elapsed_secs: f32,
) {
    push_hud_feedback(
        feedback,
        command_error_message(err).to_string(),
        elapsed_secs,
        true,
    );
}

/// Crea el toast una vez por sesión. El panel se posiciona sobre la barra de
/// estado, lejos del toolbar y de los paneles principales.
pub(crate) fn setup_hud_feedback_toast(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            InGameUi,
            HudFeedbackToastRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(HUD_FEEDBACK_BOTTOM),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            FocusPolicy::Pass,
            GlobalZIndex(HUD_FEEDBACK_Z),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(92.0),
                    max_width: Val::Px(680.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(FEEDBACK_BG),
                BorderColor::all(FEEDBACK_BORDER),
                FocusPolicy::Pass,
            ))
            .with_children(|panel| {
                panel.spawn((
                    HudFeedbackToastText,
                    Text::new(""),
                    window_text_font(&asset_server, UiFontRole::Body),
                    TextColor(FEEDBACK_TEXT),
                    Node {
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    FocusPolicy::Pass,
                ));
            });
        });
}

/// Sincroniza y vence el aviso temporal. Esta ruta no depende de
/// [`crate::ui::hud::HudVisibility`], por eso guardar/cargar se confirma aun
/// cuando el jugador eligió ocultar el HUD informativo o pausó la simulación.
pub(crate) fn sync_hud_feedback_toast(
    mut feedback: ResMut<HudBuildFeedback>,
    time: Res<Time>,
    prefs: Res<crate::settings::ClientPreferences>,
    mut roots: Query<&mut Visibility, With<HudFeedbackToastRoot>>,
    mut texts: Query<&mut Text, With<HudFeedbackToastText>>,
) {
    if feedback.message.is_some() && time.elapsed_secs() >= feedback.expires_at_secs {
        feedback.message = None;
        feedback.pending_soft_ping = false;
    }

    let message = feedback
        .message
        .as_deref()
        .map(|message| localized_text(prefs.locale(), message));
    for mut visibility in &mut roots {
        *visibility = if message.is_some() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    let Some(message) = message else {
        return;
    };
    for mut text in &mut texts {
        if text.as_str() != message {
            *text = Text::new(message.clone());
        }
    }
}

/// Muestra un rechazo CB31 con el texto del catálogo activo cuando está
/// disponible. El diagnóstico se consume aunque la cadena falte para que no
/// pueda reaparecer en otro vehículo o en otro comando.
pub(crate) fn push_vehicle_start_stop_error(
    feedback: &mut HudBuildFeedback,
    sim: &mut SimWorld,
    err: CommandError,
    vehicle_id: u32,
    locale: Locale,
    elapsed_secs: f32,
) {
    let diagnostic = sim.state.runtime.last_vehicle_start_stop_diagnostic.take();
    let dynamic_message = if matches!(err, CommandError::NewGrfCallbackDenied) {
        diagnostic
            .filter(|diagnostic| diagnostic.vehicle_id == vehicle_id)
            .and_then(|diagnostic| {
                let string_id = match diagnostic.outcome {
                    openttdrs_core::VehicleStartStopCallbackOutcome::LocalString(string_id)
                    | openttdrs_core::VehicleStartStopCallbackOutcome::GrfString(string_id) => {
                        string_id
                    }
                    openttdrs_core::VehicleStartStopCallbackOutcome::Allow
                    | openttdrs_core::VehicleStartStopCallbackOutcome::GenericDenied(_) => {
                        return None;
                    }
                };
                let language = match locale {
                    Locale::Es => openttdrs_core::NEWGRF_LANGUAGE_SPANISH,
                    Locale::En => openttdrs_core::NEWGRF_LANGUAGE_ENGLISH,
                };
                let text = sim.state.runtime.newgrf_string_catalog.lookup_rendered(
                    diagnostic.grfid,
                    string_id,
                    language,
                    &openttdrs_core::NewGrfTextContext::default(),
                )?;
                if text.is_empty() {
                    return None;
                }
                let prefix = match locale {
                    Locale::Es => "Un NewGRF denegó esta acción",
                    Locale::En => "A NewGRF denied this action",
                };
                Some(format!("{prefix}: {text}"))
            })
    } else {
        None
    };

    feedback.message =
        Some(dynamic_message.unwrap_or_else(|| command_error_message(err).to_string()));
    feedback.expires_at_secs = elapsed_secs + HUD_FEEDBACK_DISPLAY_SECS;
    feedback.pending_soft_ping = true;
}

/// Muestra un rechazo de CB149 con el texto del catálogo activo cuando el
/// callback devolvió un motivo NewGRF. El diagnóstico se consume siempre para
/// que no sobreviva al comando que lo produjo.
pub(crate) fn push_station_slope_error(
    feedback: &mut HudBuildFeedback,
    sim: &mut SimWorld,
    err: CommandError,
    locale: Locale,
    elapsed_secs: f32,
) {
    let diagnostic = sim.state.runtime.last_station_slope_diagnostic.take();
    let dynamic_message = if matches!(err, CommandError::NewGrfCallbackDenied) {
        diagnostic.and_then(|diagnostic| {
            let string_id = match diagnostic.outcome {
                openttdrs_core::StationSlopeCallbackOutcome::LocalString(string_id)
                | openttdrs_core::StationSlopeCallbackOutcome::GrfString(string_id) => string_id,
                openttdrs_core::StationSlopeCallbackOutcome::Allow => return None,
                openttdrs_core::StationSlopeCallbackOutcome::GenericDenied(code) => {
                    return standard_station_slope_error(code, locale);
                }
            };
            let language = match locale {
                Locale::Es => openttdrs_core::NEWGRF_LANGUAGE_SPANISH,
                Locale::En => openttdrs_core::NEWGRF_LANGUAGE_ENGLISH,
            };
            let text = sim.state.runtime.newgrf_string_catalog.lookup_rendered(
                diagnostic.grfid,
                string_id,
                language,
                &openttdrs_core::NewGrfTextContext::default(),
            )?;
            if text.is_empty() {
                return None;
            }
            let prefix = match locale {
                Locale::Es => "La estación no puede construirse",
                Locale::En => "The station cannot be built",
            };
            Some(format!("{prefix}: {text}"))
        })
    } else {
        None
    };

    feedback.message =
        Some(dynamic_message.unwrap_or_else(|| command_error_message(err).to_string()));
    feedback.expires_at_secs = elapsed_secs + HUD_FEEDBACK_DISPLAY_SECS;
    feedback.pending_soft_ping = true;
}

/// Muestra un rechazo de CB157 con el texto del catálogo activo. El
/// diagnóstico se consume siempre para que un fallo posterior no reutilice el
/// motivo de otro objeto.
pub(crate) fn push_object_slope_error(
    feedback: &mut HudBuildFeedback,
    sim: &mut SimWorld,
    err: CommandError,
    locale: Locale,
    elapsed_secs: f32,
) {
    let diagnostic = sim.state.runtime.last_object_slope_diagnostic.take();
    let dynamic_message = if matches!(err, CommandError::NewGrfCallbackDenied) {
        diagnostic.and_then(|diagnostic| {
            let string_id = match diagnostic.outcome {
                openttdrs_core::ObjectSlopeCallbackOutcome::LocalString(string_id)
                | openttdrs_core::ObjectSlopeCallbackOutcome::GrfString(string_id) => string_id,
                openttdrs_core::ObjectSlopeCallbackOutcome::Allow => return None,
                openttdrs_core::ObjectSlopeCallbackOutcome::GenericDenied(code) => {
                    return standard_object_slope_error(code, locale);
                }
            };
            let language = match locale {
                Locale::Es => openttdrs_core::NEWGRF_LANGUAGE_SPANISH,
                Locale::En => openttdrs_core::NEWGRF_LANGUAGE_ENGLISH,
            };
            let text = sim.state.runtime.newgrf_string_catalog.lookup_rendered(
                diagnostic.grfid,
                string_id,
                language,
                &openttdrs_core::NewGrfTextContext::default(),
            )?;
            if text.is_empty() {
                return None;
            }
            let prefix = match locale {
                Locale::Es => "El objeto no puede construirse",
                Locale::En => "The object cannot be built",
            };
            Some(format!("{prefix}: {text}"))
        })
    } else {
        None
    };

    feedback.message =
        Some(dynamic_message.unwrap_or_else(|| command_error_message(err).to_string()));
    feedback.expires_at_secs = elapsed_secs + HUD_FEEDBACK_DISPLAY_SECS;
    feedback.pending_soft_ping = true;
}

fn standard_station_slope_error(code: u16, locale: Locale) -> Option<String> {
    let message = match (locale, code) {
        (Locale::Es, 0x402) => "Sólo se puede construir en selva.",
        (Locale::Es, 0x403) => "Sólo se puede construir en desierto.",
        (Locale::Es, 0x404) => "Sólo se puede construir por encima de la línea de nieve.",
        (Locale::Es, 0x405) => "Sólo se puede construir por debajo de la línea de nieve.",
        (Locale::Es, 0x406) => "No se puede construir en el mar.",
        (Locale::Es, 0x407) => "No se puede construir sobre un canal.",
        (Locale::Es, 0x408) => "No se puede construir sobre un río.",
        (Locale::En, 0x402) => "This can only be built in rainforest.",
        (Locale::En, 0x403) => "This can only be built in desert.",
        (Locale::En, 0x404) => "This can only be built above the snow line.",
        (Locale::En, 0x405) => "This can only be built below the snow line.",
        (Locale::En, 0x406) => "This cannot be built on sea.",
        (Locale::En, 0x407) => "This cannot be built on a canal.",
        (Locale::En, 0x408) => "This cannot be built on a river.",
        _ => return None,
    };
    Some(message.to_string())
}

fn standard_object_slope_error(code: u16, locale: Locale) -> Option<String> {
    let message = match (locale, code) {
        (Locale::Es, 0x402) => "Sólo se puede construir en selva.",
        (Locale::Es, 0x403) => "Sólo se puede construir en desierto.",
        (Locale::Es, 0x404) => "Sólo se puede construir por encima de la línea de nieve.",
        (Locale::Es, 0x405) => "Sólo se puede construir por debajo de la línea de nieve.",
        (Locale::Es, 0x406) => "No se puede construir en el mar.",
        (Locale::Es, 0x407) => "No se puede construir sobre un canal.",
        (Locale::Es, 0x408) => "No se puede construir sobre un río.",
        (Locale::En, 0x402) => "This can only be built in rainforest.",
        (Locale::En, 0x403) => "This can only be built in desert.",
        (Locale::En, 0x404) => "This can only be built above the snow line.",
        (Locale::En, 0x405) => "This can only be built below the snow line.",
        (Locale::En, 0x406) => "This cannot be built on sea.",
        (Locale::En, 0x407) => "This cannot be built on a canal.",
        (Locale::En, 0x408) => "This cannot be built on a river.",
        _ => return None,
    };
    Some(message.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::time::Duration;

    use super::super::HudVisibility;
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use openttdrs_core::{
        NewGrfString, ObjectSlopeCallbackDiagnostic, ObjectSlopeCallbackOutcome,
        StationSlopeCallbackDiagnostic, StationSlopeCallbackOutcome,
        VehicleStartStopCallbackDiagnostic, VehicleStartStopCallbackOutcome,
    };

    use crate::settings::ClientPreferences;
    use crate::state::SimRunState;

    #[test]
    fn toast_is_visible_while_paused_and_hides_after_expiring() {
        let mut world = World::new();
        world.insert_resource(ClientPreferences {
            language: "en".into(),
            ..ClientPreferences::default()
        });
        world.insert_resource(HudVisibility::default());
        world.insert_resource(HudBuildFeedback {
            message: Some("Game saved: paused.json".into()),
            expires_at_secs: 5.0,
            ..Default::default()
        });
        world.insert_resource(Time::<()>::default());
        world.insert_resource(State::new(SimRunState::Paused));
        let root = world.spawn((HudFeedbackToastRoot, Visibility::Hidden)).id();
        let text = world.spawn((HudFeedbackToastText, Text::new(""))).id();

        world.run_system_once(sync_hud_feedback_toast).unwrap();

        assert_eq!(
            world.resource::<State<SimRunState>>().get(),
            &SimRunState::Paused
        );
        assert!(!world.resource::<HudVisibility>().visible);
        assert!(matches!(
            world.get::<Visibility>(root),
            Some(Visibility::Visible)
        ));
        assert_eq!(
            world.get::<Text>(text).expect("toast text").as_str(),
            "Game saved: paused.json"
        );

        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(5));
        world.run_system_once(sync_hud_feedback_toast).unwrap();

        assert!(world.resource::<HudBuildFeedback>().message.is_none());
        assert!(matches!(
            world.get::<Visibility>(root),
            Some(Visibility::Hidden)
        ));
    }

    #[test]
    fn vehicle_start_stop_error_uses_expanded_catalog_text_and_consumes_diagnostic() {
        let mut sim = SimWorld::default();
        sim.state.runtime.newgrf_string_catalog.push(NewGrfString {
            grfid: 7,
            string_id: 0xD010,
            language: openttdrs_core::NEWGRF_LANGUAGE_SPANISH,
            text: "Motivo ⟦grf-string:0x0001⟧".into(),
        });
        sim.state.runtime.newgrf_string_catalog.push(NewGrfString {
            grfid: 7,
            string_id: 0xD001,
            language: openttdrs_core::NEWGRF_LANGUAGE_SPANISH,
            text: "específico".into(),
        });
        sim.state.runtime.last_vehicle_start_stop_diagnostic =
            Some(VehicleStartStopCallbackDiagnostic {
                vehicle_id: 42,
                grfid: 7,
                outcome: VehicleStartStopCallbackOutcome::LocalString(0xD010),
            });
        let mut feedback = HudBuildFeedback::default();

        push_vehicle_start_stop_error(
            &mut feedback,
            &mut sim,
            CommandError::NewGrfCallbackDenied,
            42,
            Locale::Es,
            10.0,
        );

        assert_eq!(
            feedback.message.as_deref(),
            Some("Un NewGRF denegó esta acción: Motivo específico")
        );
        assert!(
            sim.state
                .runtime
                .last_vehicle_start_stop_diagnostic
                .is_none()
        );
    }

    #[test]
    fn vehicle_start_stop_error_keeps_generic_message_for_missing_text() {
        let mut sim = SimWorld::default();
        sim.state.runtime.last_vehicle_start_stop_diagnostic =
            Some(VehicleStartStopCallbackDiagnostic {
                vehicle_id: 42,
                grfid: 7,
                outcome: VehicleStartStopCallbackOutcome::GenericDenied(0x401),
            });
        let mut feedback = HudBuildFeedback::default();

        push_vehicle_start_stop_error(
            &mut feedback,
            &mut sim,
            CommandError::NewGrfCallbackDenied,
            42,
            Locale::En,
            10.0,
        );

        assert_eq!(
            feedback.message.as_deref(),
            Some("Un NewGRF denegó esta acción (callback).")
        );
        assert!(
            sim.state
                .runtime
                .last_vehicle_start_stop_diagnostic
                .is_none()
        );
    }

    #[test]
    fn station_slope_error_uses_catalog_text_and_consumes_diagnostic() {
        let mut sim = SimWorld::default();
        sim.state.runtime.newgrf_string_catalog.push(NewGrfString {
            grfid: 9,
            string_id: 0xD002,
            language: openttdrs_core::NEWGRF_LANGUAGE_ENGLISH,
            text: "Use una plataforma plana".into(),
        });
        sim.state.runtime.last_station_slope_diagnostic = Some(StationSlopeCallbackDiagnostic {
            grfid: 9,
            outcome: StationSlopeCallbackOutcome::LocalString(0xD002),
        });
        let mut feedback = HudBuildFeedback::default();

        push_station_slope_error(
            &mut feedback,
            &mut sim,
            CommandError::NewGrfCallbackDenied,
            Locale::En,
            3.0,
        );

        assert_eq!(
            feedback.message.as_deref(),
            Some("The station cannot be built: Use una plataforma plana")
        );
        assert!(sim.state.runtime.last_station_slope_diagnostic.is_none());
    }

    #[test]
    fn station_slope_error_localizes_standard_callback_codes() {
        let mut sim = SimWorld::default();
        sim.state.runtime.last_station_slope_diagnostic = Some(StationSlopeCallbackDiagnostic {
            grfid: 9,
            outcome: StationSlopeCallbackOutcome::GenericDenied(0x407),
        });
        let mut feedback = HudBuildFeedback::default();

        push_station_slope_error(
            &mut feedback,
            &mut sim,
            CommandError::NewGrfCallbackDenied,
            Locale::Es,
            3.0,
        );

        assert_eq!(
            feedback.message.as_deref(),
            Some("No se puede construir sobre un canal.")
        );
    }

    #[test]
    fn object_slope_error_uses_expanded_catalog_text_and_consumes_diagnostic() {
        let mut sim = SimWorld::default();
        sim.state.runtime.newgrf_string_catalog.push(NewGrfString {
            grfid: 12,
            string_id: 0xD003,
            language: openttdrs_core::NEWGRF_LANGUAGE_SPANISH,
            text: "El terreno no es válido".into(),
        });
        sim.state.runtime.last_object_slope_diagnostic = Some(ObjectSlopeCallbackDiagnostic {
            grfid: 12,
            outcome: ObjectSlopeCallbackOutcome::LocalString(0xD003),
        });
        let mut feedback = HudBuildFeedback::default();

        push_object_slope_error(
            &mut feedback,
            &mut sim,
            CommandError::NewGrfCallbackDenied,
            Locale::Es,
            2.0,
        );

        assert_eq!(
            feedback.message.as_deref(),
            Some("El objeto no puede construirse: El terreno no es válido")
        );
        assert!(sim.state.runtime.last_object_slope_diagnostic.is_none());
    }

    #[test]
    fn object_slope_error_localizes_standard_callback_codes() {
        let mut sim = SimWorld::default();
        sim.state.runtime.last_object_slope_diagnostic = Some(ObjectSlopeCallbackDiagnostic {
            grfid: 12,
            outcome: ObjectSlopeCallbackOutcome::GenericDenied(0x40F),
        });
        let mut feedback = HudBuildFeedback::default();

        push_object_slope_error(
            &mut feedback,
            &mut sim,
            CommandError::NewGrfCallbackDenied,
            Locale::En,
            2.0,
        );

        assert_eq!(
            feedback.message.as_deref(),
            Some("Un NewGRF denegó esta acción (callback).")
        );
        assert!(sim.state.runtime.last_object_slope_diagnostic.is_none());
    }
}
