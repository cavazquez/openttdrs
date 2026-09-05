use bevy::prelude::*;

use crate::i18n::{Locale, localized_text};
use crate::settings::ClientPreferences;
use crate::ui::toolbar::{ToolbarTooltipTarget, TooltipBox, TooltipText};

pub(crate) fn update_toolbar_tooltip(
    mut tooltip_q: Query<&mut Node, With<TooltipBox>>,
    mut text_q: Query<&mut Text, With<TooltipText>>,
    target_q: Query<(&Interaction, &ToolbarTooltipTarget)>,
    prefs: Option<Res<ClientPreferences>>,
) {
    let mut hovered: Option<&'static str> = None;
    for (interaction, tip) in &target_q {
        if *interaction == Interaction::Hovered {
            hovered = Some(tip.text);
            break;
        }
    }

    let Ok(mut tooltip_text) = text_q.single_mut() else {
        return;
    };
    let Ok(mut node) = tooltip_q.single_mut() else {
        return;
    };

    if let Some(text) = hovered {
        let locale = prefs.as_ref().map_or(Locale::Es, |prefs| prefs.locale());
        **tooltip_text = localized_text(locale, text);
        node.display = Display::Flex;
    } else {
        node.display = Display::None;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn hovered_tooltip_uses_active_locale() {
        let mut world = World::new();
        world.insert_resource(ClientPreferences {
            language: "en".into(),
            ..ClientPreferences::default()
        });
        let tooltip = world.spawn((TooltipText, Text::new(""))).id();
        world.spawn((TooltipBox, Node::default()));
        world.spawn((
            Interaction::Hovered,
            ToolbarTooltipTarget { text: "Cerrar" },
        ));

        world.run_system_once(update_toolbar_tooltip).unwrap();

        assert_eq!(world.get::<Text>(tooltip).unwrap().as_str(), "Close");
    }
}
