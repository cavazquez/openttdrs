//! Recurso Bevy con opciones de nueva partida (menú principal).

use std::time::{SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

use crate::state::bootstrap::{MapSizePreset, NewGameSettings, START_YEARS};

/// Alias del recurso en el menú (evita colisión con `NewGameSettings` del bootstrap).
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct NewGameSettingsResource(pub NewGameSettings);

impl Default for NewGameSettingsResource {
    fn default() -> Self {
        Self(NewGameSettings {
            map_size: MapSizePreset::SMALL,
            start_year: START_YEARS[0],
            world_gen: true,
            island: true,
            preserve_demo: false,
            seed: 0,
            ..NewGameSettings::default()
        })
    }
}

impl NewGameSettingsResource {
    #[must_use]
    pub const fn settings(self) -> NewGameSettings {
        self.0
    }
}

/// Secuencia de semillas para la opción automática del menú.
///
/// La configuración mantiene `seed: 0` para mostrar «auto» y no alterar la
/// elección guardada del usuario. Al crear el mundo se consume una semilla de
/// esta secuencia, distinta para cada partida de la misma ejecución.
#[derive(Resource, Debug)]
pub struct NewGameSeedSequence {
    state: u64,
}

impl Default for NewGameSeedSequence {
    fn default() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let entropy = elapsed.as_secs().rotate_left(32)
            ^ u64::from(elapsed.subsec_nanos())
            ^ u64::from(std::process::id()).rotate_left(17);
        Self {
            state: if entropy == 0 {
                0xA5A5_5A5A_F0F0_0F0F
            } else {
                entropy
            },
        }
    }
}

impl NewGameSeedSequence {
    /// Devuelve una semilla no nula y no repetida hasta completar el período
    /// del generador xorshift64.
    #[must_use]
    pub fn next_seed(&mut self) -> u64 {
        let mut state = self.state;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        if state == 0 {
            state = 0xA5A5_5A5A_F0F0_0F0F;
        }
        self.state = state;
        state
    }
}

#[cfg(test)]
mod tests {
    use super::NewGameSeedSequence;

    #[test]
    fn automatic_seed_sequence_is_nonzero_and_unique() {
        let mut sequence = NewGameSeedSequence { state: 1 };
        let first = sequence.next_seed();
        let second = sequence.next_seed();

        assert_ne!(first, 0);
        assert_ne!(second, 0);
        assert_ne!(first, second);
    }
}
