//! Hash canónico del estado persistido (#108).
//!
//! Serializa `GameState` vía `serde` (excluye `runtime`), ordena claves de
//! objetos JSON (incluye `HashMap`/`HashSet` convertidos a mapa) y aplica
//! FNV-1a 64 con dominio versionado. No usa el texto de `save_json`.

use serde_json::Value;

use super::GameState;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;
const DOMAIN: &[u8] = b"openttdrs-gs-v1";

#[derive(Debug, Clone, Copy)]
struct Fnv1a64(u64);

impl Fnv1a64 {
    fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn write_u8(&mut self, v: u8) {
        self.0 ^= u64::from(v);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u8(*b);
        }
    }

    fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

impl GameState {
    /// Fingerprint estable del estado **persistido** (excluye `runtime`).
    ///
    /// Mismo seed/comandos/ticks ⇒ mismo hash. Útil para desync (#21) y
    /// equivalencia de refactors. El dominio `openttdrs-gs-v1` versiona el
    /// algoritmo; un cambio de formato debe subir la etiqueta.
    ///
    /// # Panics
    ///
    /// Si la serialización serde falla (no debería con el esquema actual).
    #[must_use]
    pub fn canonical_hash(&self) -> u64 {
        let Ok(value) = serde_json::to_value(self) else {
            // El esquema de `GameState` es serializable; un fallo indica bug de tipos.
            panic!("GameState serializable for canonical_hash");
        };
        let mut hasher = Fnv1a64::new();
        hasher.write_bytes(DOMAIN);
        hash_value(&value, &mut hasher);
        hasher.finish()
    }
}

fn hash_value(value: &Value, hasher: &mut Fnv1a64) {
    match value {
        Value::Null => hasher.write_u8(0),
        Value::Bool(b) => {
            hasher.write_u8(1);
            hasher.write_u8(u8::from(*b));
        }
        Value::Number(n) => {
            hasher.write_u8(2);
            if let Some(i) = n.as_i64() {
                hasher.write_u8(0);
                hasher.write_bytes(&i.to_le_bytes());
            } else if let Some(u) = n.as_u64() {
                hasher.write_u8(1);
                hasher.write_u64(u);
            } else if let Some(f) = n.as_f64() {
                hasher.write_u8(2);
                hasher.write_bytes(&f.to_bits().to_le_bytes());
            } else {
                hasher.write_u8(3);
                hasher.write_bytes(n.to_string().as_bytes());
            }
        }
        Value::String(s) => {
            hasher.write_u8(3);
            hasher.write_u64(s.len() as u64);
            hasher.write_bytes(s.as_bytes());
        }
        Value::Array(items) => {
            hasher.write_u8(4);
            hasher.write_u64(items.len() as u64);
            for item in items {
                hash_value(item, hasher);
            }
        }
        Value::Object(map) => {
            hasher.write_u8(5);
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            hasher.write_u64(keys.len() as u64);
            for key in keys {
                hasher.write_u64(key.len() as u64);
                hasher.write_bytes(key.as_bytes());
                hash_value(&map[key], hasher);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::{Command, apply_command};
    use crate::map::TileCoord;
    use crate::parity::build_truck_bay;

    #[test]
    fn empty_worlds_share_hash() {
        let a = GameState::new(8, 8);
        let b = GameState::new(8, 8);
        assert_eq!(a.canonical_hash(), b.canonical_hash());
    }

    #[test]
    fn same_scenario_same_ticks_same_hash() {
        let mut a = build_truck_bay();
        let mut b = build_truck_bay();
        for _ in 0..120 {
            a.step();
            b.step();
        }
        assert_eq!(a.canonical_hash(), b.canonical_hash());
    }

    #[test]
    fn save_load_mid_run_preserves_hash_trajectory() {
        let mut control = build_truck_bay();
        let mut subject = build_truck_bay();
        for _ in 0..60 {
            control.step();
            subject.step();
        }
        let mid = subject.save_json().unwrap();
        let mut subject = GameState::load_json(&mid).unwrap();
        for _ in 0..60 {
            control.step();
            subject.step();
        }
        assert_eq!(control.canonical_hash(), subject.canonical_hash());
    }

    #[test]
    fn deliberate_mutation_changes_hash() {
        let mut a = build_truck_bay();
        let mut b = build_truck_bay();
        for _ in 0..40 {
            a.step();
            b.step();
        }
        let before = a.canonical_hash();
        assert_eq!(before, b.canonical_hash());
        b.economy.money = b.economy.money.saturating_add(1);
        assert_ne!(before, b.canonical_hash());
    }

    #[test]
    fn command_then_steps_is_repeatable() {
        let cmds = [
            Command::PlaceRail(TileCoord::new(2, 2)),
            Command::PlaceRail(TileCoord::new(3, 2)),
        ];
        let mut a = GameState::new(16, 16);
        let mut b = GameState::new(16, 16);
        for cmd in &cmds {
            apply_command(&mut a, cmd).unwrap();
            apply_command(&mut b, cmd).unwrap();
        }
        for _ in 0..30 {
            a.step();
            b.step();
        }
        assert_eq!(a.canonical_hash(), b.canonical_hash());
    }
}
