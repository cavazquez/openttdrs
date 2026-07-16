//! Banner de “reconectando…” / “promoviendo host…” durante failover (#171).

use bevy::prelude::*;

use crate::network::failover::FailoverUiPhase;
use crate::network::plugin::NetworkStatus;
use crate::ui::font::{HudUiFont, UiFontRole, ui_text_font};

#[derive(Component)]
pub(crate) struct NetworkFailoverBanner;

#[derive(Component)]
pub(crate) struct NetworkFailoverBannerText;

fn phase_message(phase: &FailoverUiPhase) -> Option<String> {
    match phase {
        FailoverUiPhase::Idle => None,
        FailoverUiPhase::Promoting { bind } => {
            Some(format!("Migración de host: escuchando en {bind}…"))
        }
        FailoverUiPhase::Reconnecting { addr, attempt } => {
            Some(format!("Reconectando a {addr}… (intento {attempt})"))
        }
        FailoverUiPhase::Failed { reason } => Some(format!("Failover fallido: {reason}")),
    }
}

/// Spawnea / actualiza / despawnea el overlay según [`NetworkStatus::failover_phase`].
pub(crate) fn sync_failover_banner(
    mut commands: Commands,
    status: Res<NetworkStatus>,
    hud_font: Option<Res<HudUiFont>>,
    mut q_banner: Query<(Entity, &mut Text), With<NetworkFailoverBannerText>>,
    q_root: Query<Entity, With<NetworkFailoverBanner>>,
) {
    let Some(message) = phase_message(&status.failover_phase) else {
        for entity in &q_root {
            commands.entity(entity).despawn();
        }
        return;
    };

    if let Ok((_, mut text)) = q_banner.single_mut() {
        if **text != message {
            *text = Text::new(message);
        }
        return;
    }

    for entity in &q_root {
        commands.entity(entity).despawn();
    }

    let text_font = match hud_font {
        Some(font) => ui_text_font(font.0.clone(), UiFontRole::Hud),
        None => TextFont {
            font_size: FontSize::Rem(UiFontRole::Hud.rem_size()),
            ..default()
        },
    };

    commands
        .spawn((
            NetworkFailoverBanner,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(48.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                height: Val::Px(44.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.1, 0.14, 0.88)),
            GlobalZIndex(2500),
        ))
        .with_children(|p| {
            p.spawn((
                NetworkFailoverBannerText,
                Text::new(message),
                text_font,
                TextColor(Color::srgb(0.96, 0.9, 0.68)),
            ));
        });
}
