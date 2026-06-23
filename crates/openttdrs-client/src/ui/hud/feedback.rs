use openttdrs_core::CommandError;

use super::HudBuildFeedback;

const BUILD_ERROR_DISPLAY_SECS: f32 = 5.0;

/// Muestra un mensaje temporal en el HUD y encola pitido suave.
pub(crate) fn push_build_command_error(
    feedback: &mut HudBuildFeedback,
    err: CommandError,
    elapsed_secs: f32,
) {
    feedback.message = Some(openttdrs_core::command_error_message(err).to_string());
    feedback.expires_at_secs = elapsed_secs + BUILD_ERROR_DISPLAY_SECS;
    feedback.pending_soft_ping = true;
}

/// Encola sonido breve tras un comando de construcción exitoso.
pub(crate) fn push_build_command_success(feedback: &mut HudBuildFeedback) {
    feedback.pending_build_ok_ping = true;
}
