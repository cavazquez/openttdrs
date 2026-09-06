use openttdrs_core::CommandError;

use super::HudBuildFeedback;
use crate::i18n::Locale;
use crate::state::SimWorld;
use crate::ui::command_error_text::command_error_message;

const BUILD_ERROR_DISPLAY_SECS: f32 = 5.0;

/// Muestra un mensaje temporal en el HUD y encola pitido suave.
pub(crate) fn push_build_command_error(
    feedback: &mut HudBuildFeedback,
    err: CommandError,
    elapsed_secs: f32,
) {
    feedback.message = Some(command_error_message(err).to_string());
    feedback.expires_at_secs = elapsed_secs + BUILD_ERROR_DISPLAY_SECS;
    feedback.pending_soft_ping = true;
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
                let text = sim.state.runtime.newgrf_string_catalog.lookup_expanded(
                    diagnostic.grfid,
                    string_id,
                    language,
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
    feedback.expires_at_secs = elapsed_secs + BUILD_ERROR_DISPLAY_SECS;
    feedback.pending_soft_ping = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use openttdrs_core::{
        NewGrfString, VehicleStartStopCallbackDiagnostic, VehicleStartStopCallbackOutcome,
    };

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
}
