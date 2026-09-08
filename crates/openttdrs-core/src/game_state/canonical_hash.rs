//! Hash canónico del estado persistido (#108).
//!
//! Serializa `GameState` vía `serde` (excluye `runtime`), ordena claves de
//! objetos JSON (incluye `HashMap`/`HashSet` convertidos a mapa) y aplica
//! FNV-1a 64 con dominio versionado. No usa el texto de `save_json`.

use serde_json::Value;

use super::GameState;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;
// v3 incorpora la cola ordenada de ascensores activos. Mantenerlo separado
// evita presentar hashes de dos algoritmos como si fueran comparables.
const DOMAIN: &[u8] = b"openttdrs-gs-v3";

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

    const ANIMATION_FIELDS: [&str; 4] = [
        "newgrf_animated_industry_tiles",
        "newgrf_animated_station_tiles",
        "newgrf_animated_airport_tiles",
        "newgrf_animated_object_tiles",
    ];

    fn animation_coordinates() -> [TileCoord; 16] {
        [
            TileCoord::new(0, 0),
            TileCoord::new(1, 5),
            TileCoord::new(2, 2),
            TileCoord::new(3, 7),
            TileCoord::new(4, 4),
            TileCoord::new(5, 1),
            TileCoord::new(6, 6),
            TileCoord::new(7, 3),
            TileCoord::new(0, 7),
            TileCoord::new(1, 2),
            TileCoord::new(2, 6),
            TileCoord::new(3, 1),
            TileCoord::new(4, 5),
            TileCoord::new(5, 0),
            TileCoord::new(6, 4),
            TileCoord::new(7, 6),
        ]
    }

    fn animation_state(
        family: usize,
        coordinates: impl IntoIterator<Item = TileCoord>,
    ) -> GameState {
        let mut state = GameState::new(8, 8);
        let tiles = coordinates.into_iter().collect();
        match family {
            0 => state.newgrf_animated_industry_tiles = tiles,
            1 => state.newgrf_animated_station_tiles = tiles,
            2 => state.newgrf_animated_airport_tiles = tiles,
            3 => state.newgrf_animated_object_tiles = tiles,
            _ => unreachable!("familia de animación conocida"),
        }
        state
    }

    fn add_animation_tile(state: &mut GameState, family: usize, tile: TileCoord) {
        match family {
            0 => {
                state.newgrf_animated_industry_tiles.insert(tile);
            }
            1 => {
                state.newgrf_animated_station_tiles.insert(tile);
            }
            2 => {
                state.newgrf_animated_airport_tiles.insert(tile);
            }
            3 => {
                state.newgrf_animated_object_tiles.insert(tile);
            }
            _ => unreachable!("familia de animación conocida"),
        }
    }

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

    #[test]
    fn persisted_animation_tile_sets_are_canonical_across_hashset_orders() {
        let coordinates = animation_coordinates();
        let reverse: Vec<_> = coordinates.iter().rev().copied().collect();
        let interleaved: Vec<_> = (0..coordinates.len())
            .step_by(2)
            .chain((1..coordinates.len()).step_by(2))
            .map(|index| coordinates[index])
            .collect();

        for (family, &field) in ANIMATION_FIELDS.iter().enumerate() {
            // Cada estado crea un HashSet con RandomState propio. Además de
            // cambiar la inserción, cubre semillas de hasher distintas.
            let forward = animation_state(family, coordinates);
            let backwards = animation_state(family, reverse.iter().copied());
            let shuffled = animation_state(family, interleaved.iter().copied());
            assert_eq!(
                forward.canonical_hash(),
                backwards.canonical_hash(),
                "{field} conserva el hash con inserción inversa"
            );
            assert_eq!(
                forward.canonical_hash(),
                shuffled.canonical_hash(),
                "{field} conserva el hash con inserción intercalada"
            );

            let serialized = serde_json::to_value(&forward).unwrap();
            let serialized_tiles: Vec<TileCoord> =
                serde_json::from_value(serialized[field].clone()).unwrap();
            assert!(
                serialized_tiles.windows(2).all(|pair| pair[0] <= pair[1]),
                "{field} se persiste ordenado"
            );
        }
    }

    #[test]
    fn animation_tile_set_membership_changes_canonical_hash() {
        let coordinates = animation_coordinates();
        for (family, &field) in ANIMATION_FIELDS.iter().enumerate() {
            let baseline = animation_state(family, coordinates);
            let mut changed = animation_state(family, coordinates);
            add_animation_tile(&mut changed, family, TileCoord::new(7, 7));
            assert_ne!(
                baseline.canonical_hash(),
                changed.canonical_hash(),
                "{field} debe participar por membresía, no sólo por cardinalidad"
            );
        }
    }

    #[test]
    fn animation_tile_sets_roundtrip_without_mutating_runtime_representation() {
        let coordinates = animation_coordinates();
        let mut state = GameState::new(8, 8);
        for (family, _) in ANIMATION_FIELDS.iter().enumerate() {
            for tile in coordinates {
                add_animation_tile(&mut state, family, tile);
            }
        }
        let original_sets = (
            state.newgrf_animated_industry_tiles.clone(),
            state.newgrf_animated_station_tiles.clone(),
            state.newgrf_animated_airport_tiles.clone(),
            state.newgrf_animated_object_tiles.clone(),
        );
        let hash = state.canonical_hash();

        let saved = state.save_json().unwrap();
        assert_eq!(
            original_sets,
            (
                state.newgrf_animated_industry_tiles.clone(),
                state.newgrf_animated_station_tiles.clone(),
                state.newgrf_animated_airport_tiles.clone(),
                state.newgrf_animated_object_tiles.clone(),
            ),
            "serializar no cambia los HashSet que consume el scheduler"
        );

        let loaded = GameState::load_json(&saved).unwrap();
        assert_eq!(hash, loaded.canonical_hash());
        assert_eq!(
            original_sets,
            (
                loaded.newgrf_animated_industry_tiles,
                loaded.newgrf_animated_station_tiles,
                loaded.newgrf_animated_airport_tiles,
                loaded.newgrf_animated_object_tiles,
            )
        );
    }

    #[test]
    fn animation_tile_sets_read_legacy_unsorted_json_arrays() {
        let coordinates = animation_coordinates();
        let legacy_order: Vec<_> = coordinates.iter().rev().copied().collect();
        let mut value = serde_json::to_value(GameState::new(8, 8)).unwrap();
        let legacy_tiles = serde_json::to_value(legacy_order).unwrap();
        for field in ANIMATION_FIELDS {
            value[field] = legacy_tiles.clone();
        }

        let loaded = GameState::load_json(&serde_json::to_string(&value).unwrap()).unwrap();
        for field in ANIMATION_FIELDS {
            let saved = serde_json::to_value(&loaded).unwrap();
            let serialized_tiles: Vec<TileCoord> =
                serde_json::from_value(saved[field].clone()).unwrap();
            assert!(
                serialized_tiles.windows(2).all(|pair| pair[0] <= pair[1]),
                "{field} acepta el array histórico y lo vuelve a emitir canónico"
            );
        }
    }
}
