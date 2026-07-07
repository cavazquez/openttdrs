//! Horarios por orden (paridad reducida con `OpenTTD` timetable).

/// Presets de espera en parada (ticks de simulación; ~37 Hz → 30 ≈ 0,8 s).
pub const WAIT_PRESETS: [u32; 5] = [0, 30, 60, 120, 300];

/// Presets de tiempo mínimo de viaje entre órdenes.
pub const TRAVEL_PRESETS: [u32; 5] = [0, 60, 120, 240, 600];

#[must_use]
pub const fn cycle_preset(presets: &[u32], current: u32) -> u32 {
    let mut i = 0;
    while i < presets.len() {
        if presets[i] == current {
            let next = (i + 1) % presets.len();
            return presets[next];
        }
        i += 1;
    }
    presets[0]
}

#[must_use]
pub fn cycle_wait_ticks(current: u32) -> u32 {
    cycle_preset(&WAIT_PRESETS, current)
}

#[must_use]
pub fn cycle_travel_ticks(current: u32) -> u32 {
    cycle_preset(&TRAVEL_PRESETS, current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_wait_wraps_to_zero() {
        assert_eq!(cycle_wait_ticks(0), 30);
        assert_eq!(cycle_wait_ticks(300), 0);
    }
}
