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
            let text = sim.state.runtime.newgrf_string_catalog.lookup_expanded(
                diagnostic.grfid,
                string_id,
                language,
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
    feedback.expires_at_secs = elapsed_secs + BUILD_ERROR_DISPLAY_SECS;
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
            let text = sim.state.runtime.newgrf_string_catalog.lookup_expanded(
                diagnostic.grfid,
                string_id,
                language,
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
    feedback.expires_at_secs = elapsed_secs + BUILD_ERROR_DISPLAY_SECS;
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
mod tests {
    use super::*;
    use openttdrs_core::{
        NewGrfString, ObjectSlopeCallbackDiagnostic, ObjectSlopeCallbackOutcome,
        StationSlopeCallbackDiagnostic, StationSlopeCallbackOutcome,
        VehicleStartStopCallbackDiagnostic, VehicleStartStopCallbackOutcome,
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
