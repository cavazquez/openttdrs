//! Núcleo de simulación: mapa por teselas, reloj de tick y estado mínimo jugable en tests.
//!
//! Este crate no depende de Bevy ni de I/O; el cliente gráfico consume [`GameState`].

#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

pub mod industry;
pub mod map;
pub mod tick;
pub mod vehicle;

pub use industry::{Industry, IndustryKind, INDUSTRY_PRODUCE_TICKS};
pub use map::{Map, MapError, Tile, TileCoord, TileKind};
pub use tick::GameTick;
pub use vehicle::{Vehicle, VehicleKind};

/// Estado global mínimo del mundo simulado.
#[derive(Debug, Clone)]
pub struct GameState {
    pub map:        Map,
    pub tick:       GameTick,
    pub industries: Vec<Industry>,
    pub vehicles:   Vec<Vehicle>,
}

impl GameState {
    #[must_use]
    pub fn new(map_width: u32, map_height: u32) -> Self {
        Self {
            map:        Map::new_flat(map_width, map_height, 1),
            tick:       GameTick::default(),
            industries: Vec::new(),
            vehicles:   Vec::new(),
        }
    }

    /// Avanza un tick de simulación (equivalente conceptual a un frame lógico del juego).
    pub fn step(&mut self) {
        self.tick.advance();
        let t = self.tick.get();
        for industry in &mut self.industries {
            industry.produce(t);
        }
        for vehicle in &mut self.vehicles {
            vehicle.step();
        }
    }
}

#[cfg(test)]
mod tests {
    use industry::INDUSTRY_PRODUCE_AMOUNT;

    use super::*;

    #[test]
    fn new_map_has_expected_dimensions() {
        let s = GameState::new(8, 8);
        assert_eq!(s.map.dimensions(), (8, 8));
    }

    #[test]
    fn step_increments_tick() {
        let mut s = GameState::new(4, 4);
        assert_eq!(s.tick.get(), 0);
        s.step();
        assert_eq!(s.tick.get(), 1);
        s.step();
        assert_eq!(s.tick.get(), 2);
    }

    #[test]
    fn tile_height_roundtrip() {
        let mut s = GameState::new(3, 3);
        let c = TileCoord::new(1, 1);
        s.map.set_height(c, 5).unwrap();
        assert_eq!(s.map.get(c).unwrap().height, 5);
    }

    #[test]
    fn tile_kind_default_is_grass() {
        let s = GameState::new(4, 4);
        for y in 0..4_i32 {
            for x in 0..4_i32 {
                let c = TileCoord::new(x, y);
                assert_eq!(s.map.get_kind(c), Some(TileKind::Grass));
            }
        }
    }

    #[test]
    fn tile_kind_roundtrip() {
        let mut s = GameState::new(4, 4);
        let c = TileCoord::new(2, 1);
        s.map.set_kind(c, TileKind::Water).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Water));
        s.map.set_kind(c, TileKind::Forest).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
        s.map.set_kind(c, TileKind::CoalField).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::CoalField));
    }

    #[test]
    fn vehicle_moves_toward_dest() {
        let mut s = GameState::new(8, 8);
        let start = TileCoord::new(0, 0);
        let dest  = TileCoord::new(5, 0);
        s.vehicles.push(Vehicle::new(0, VehicleKind::Truck, start, dest));

        let dist_before = s.vehicles[0].manhattan_to_dest();
        s.step();
        let dist_after = s.vehicles[0].manhattan_to_dest();
        assert!(dist_after < dist_before, "debe acercarse al destino");
    }

    #[test]
    fn vehicle_inverts_on_arrival() {
        let mut s = GameState::new(8, 8);
        let start = TileCoord::new(0, 0);
        let dest  = TileCoord::new(3, 0);
        s.vehicles.push(Vehicle::new(0, VehicleKind::Truck, start, dest));

        // Avanzar hasta llegar al destino (3 pasos + 1 de inversión).
        for _ in 0..=3 {
            s.step();
        }
        assert_eq!(s.vehicles[0].pos, dest);
        // Ahora el destino debe ser el origen original.
        assert_eq!(s.vehicles[0].dest, start);

        // Avanzar de vuelta hasta el origen.
        for _ in 0..=3 {
            s.step();
        }
        assert_eq!(s.vehicles[0].pos, start);
        assert_eq!(s.vehicles[0].dest, dest);
    }

    #[test]
    fn two_worlds_same_vehicles_same_position() {
        let start = TileCoord::new(0, 0);
        let dest  = TileCoord::new(4, 3);
        let mut a = GameState::new(8, 8);
        let mut b = GameState::new(8, 8);
        for s in [&mut a, &mut b] {
            s.vehicles.push(Vehicle::new(0, VehicleKind::Truck, start, dest));
        }
        for _ in 0..50 {
            a.step();
            b.step();
        }
        assert_eq!(a.vehicles[0].pos, b.vehicles[0].pos);
    }

    #[test]
    fn industry_produces_on_schedule() {
        let mut s = GameState::new(8, 8);
        s.industries.push(Industry::new(TileCoord::new(0, 0), IndustryKind::CoalMine));

        // Sin ticks no hay producción.
        assert_eq!(s.industries[0].stock, 0);

        // Avanzar exactamente INDUSTRY_PRODUCE_TICKS ticks.
        for _ in 0..INDUSTRY_PRODUCE_TICKS {
            s.step();
        }
        assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT);

        // Un segundo ciclo completo.
        for _ in 0..INDUSTRY_PRODUCE_TICKS {
            s.step();
        }
        assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT * 2);
    }

    #[test]
    fn industry_does_not_exceed_capacity() {
        let mut s = GameState::new(8, 8);
        let mut ind = Industry::new(TileCoord::new(0, 0), IndustryKind::Forest);
        ind.capacity = INDUSTRY_PRODUCE_AMOUNT; // capacidad mínima: un ciclo
        s.industries.push(ind);

        // Primer ciclo llena hasta capacity.
        for _ in 0..INDUSTRY_PRODUCE_TICKS {
            s.step();
        }
        assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT);

        // Segundo ciclo: stock saturado, no supera capacity.
        for _ in 0..INDUSTRY_PRODUCE_TICKS {
            s.step();
        }
        assert_eq!(s.industries[0].stock, INDUSTRY_PRODUCE_AMOUNT);
    }

    #[test]
    fn two_worlds_same_industries_same_stock() {
        let mut a = GameState::new(8, 8);
        let mut b = GameState::new(8, 8);
        for state in [&mut a, &mut b] {
            state.industries.push(Industry::new(TileCoord::new(1, 2), IndustryKind::CoalMine));
            state.industries.push(Industry::new(TileCoord::new(3, 4), IndustryKind::Forest));
        }
        for _ in 0..INDUSTRY_PRODUCE_TICKS * 3 {
            a.step();
            b.step();
        }
        assert_eq!(a.industries[0].stock, b.industries[0].stock);
        assert_eq!(a.industries[1].stock, b.industries[1].stock);
    }

    #[test]
    fn tile_height_and_kind_are_independent() {
        let mut s = GameState::new(4, 4);
        let c = TileCoord::new(1, 2);
        s.map.set_height(c, 7).unwrap();
        s.map.set_kind(c, TileKind::Forest).unwrap();
        assert_eq!(s.map.get(c).unwrap().height, 7);
        assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
        // Cambiar altura no afecta el tipo.
        s.map.set_height(c, 3).unwrap();
        assert_eq!(s.map.get_kind(c), Some(TileKind::Forest));
        // Cambiar tipo no afecta la altura.
        s.map.set_kind(c, TileKind::Water).unwrap();
        assert_eq!(s.map.get(c).unwrap().height, 3);
    }
}
