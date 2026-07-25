//! Órdenes de vehículos: estaciones, waypoints, depósitos, condicionales.

use crate::cargo::CargoType;
use crate::map::TileCoord;

/// Nombre personalizado del jugador (`OpenTTD` `MAX_LENGTH_VEHICLE_NAME_CHARS` = 32).
pub const MAX_VEHICLE_NAME_CHARS: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum VehicleOrder {
    Station {
        station: TileCoord,
        /// Esperar carga completa antes de salir (`OrderLoadType::FullLoad` / `FullLoadAny`).
        #[serde(default)]
        full_load: bool,
        /// No descargar en esta parada (`OrderUnloadType::NoUnload`).
        #[serde(default)]
        no_unload: bool,
        /// Trasbordo forzado (`OrderUnloadType::Transfer`): acumula feeder, no cobra.
        #[serde(default)]
        transfer: bool,
        /// Espera mínima en parada con horario activo (ticks de sim).
        #[serde(default)]
        wait_ticks: u32,
        /// Tiempo mínimo de viaje hasta esta orden (ticks desde la anterior).
        #[serde(default)]
        travel_ticks: u32,
    },
    Waypoint {
        waypoint: TileCoord,
        #[serde(default)]
        travel_ticks: u32,
    },
    /// Parada en depósito (`stop`: detener al llegar; `false` = pasar sin parar).
    Depot {
        depot: TileCoord,
        #[serde(default = "default_depot_stop")]
        stop: bool,
        #[serde(default)]
        wait_ticks: u32,
        #[serde(default)]
        travel_ticks: u32,
        /// Refit automático al llegar (solo si `stop` y sin carga a bordo).
        #[serde(default)]
        refit_cargo: Option<CargoType>,
    },
    Tile(TileCoord),
    /// Salta a `jump_to` si la condición se cumple al llegar a esta «orden».
    Conditional {
        condition: OrderConditionKind,
        value: u8,
        jump_to: usize,
    },
}

/// Condición de orden condicional (MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderConditionKind {
    CargoLoadAbove,
    CargoLoadBelow,
}

const fn default_depot_stop() -> bool {
    true
}

impl VehicleOrder {
    #[must_use]
    pub const fn destination(self) -> TileCoord {
        match self {
            Self::Station { station, .. } => station,
            Self::Waypoint { waypoint, .. } => waypoint,
            Self::Depot { depot, .. } => depot,
            Self::Tile(pos) => pos,
            Self::Conditional { .. } => TileCoord::new(0, 0),
        }
    }

    #[must_use]
    pub const fn conditional(condition: OrderConditionKind, value: u8, jump_to: usize) -> Self {
        Self::Conditional {
            condition,
            value,
            jump_to,
        }
    }

    #[must_use]
    pub const fn is_conditional(self) -> bool {
        matches!(self, Self::Conditional { .. })
    }

    #[must_use]
    pub fn evaluate_conditional(self, vehicle: &super::model::Vehicle) -> usize {
        let Self::Conditional {
            condition,
            value,
            jump_to,
        } = self
        else {
            return vehicle.current_order;
        };
        let pct = if vehicle.capacity == 0 {
            0
        } else {
            u8::try_from((u64::from(vehicle.cargo) * 100 / u64::from(vehicle.capacity)).min(100))
                .unwrap_or(100)
        };
        let ok = match condition {
            OrderConditionKind::CargoLoadAbove => pct > value,
            OrderConditionKind::CargoLoadBelow => pct < value,
        };
        if ok {
            jump_to
        } else if vehicle.orders.is_empty() {
            0
        } else {
            (vehicle.current_order + 1) % vehicle.orders.len()
        }
    }

    #[must_use]
    pub const fn station(station: TileCoord) -> Self {
        Self::Station {
            station,
            full_load: false,
            no_unload: false,
            transfer: false,
            wait_ticks: 0,
            travel_ticks: 0,
        }
    }

    /// Siguiente estación distinta en la lista circular (`CargoDist` Manual / `next_hop`).
    #[must_use]
    pub fn next_station_hop(
        orders: &[Self],
        current_order: usize,
        from: TileCoord,
    ) -> Option<TileCoord> {
        if orders.is_empty() {
            return None;
        }
        let n = orders.len();
        let start = current_order.min(n.saturating_sub(1));
        for offset in 1..=n {
            let idx = (start + offset) % n;
            if let Self::Station { station, .. } = orders[idx]
                && station != from
            {
                return Some(station);
            }
        }
        None
    }

    #[must_use]
    pub const fn station_with_flags(station: TileCoord, full_load: bool, no_unload: bool) -> Self {
        Self::Station {
            station,
            full_load,
            no_unload,
            transfer: false,
            wait_ticks: 0,
            travel_ticks: 0,
        }
    }

    #[must_use]
    pub const fn waypoint(waypoint: TileCoord) -> Self {
        Self::Waypoint {
            waypoint,
            travel_ticks: 0,
        }
    }

    #[must_use]
    pub const fn depot(depot: TileCoord) -> Self {
        Self::Depot {
            depot,
            stop: true,
            wait_ticks: 0,
            travel_ticks: 0,
            refit_cargo: None,
        }
    }

    #[must_use]
    pub const fn depot_pass_through(depot: TileCoord) -> Self {
        Self::Depot {
            depot,
            stop: false,
            wait_ticks: 0,
            travel_ticks: 0,
            refit_cargo: None,
        }
    }

    #[must_use]
    pub const fn tile(tile: TileCoord) -> Self {
        Self::Tile(tile)
    }

    #[must_use]
    pub const fn is_depot(self) -> bool {
        matches!(self, Self::Depot { .. })
    }

    #[must_use]
    pub const fn depot_stops(self) -> bool {
        matches!(self, Self::Depot { stop: true, .. })
    }

    #[must_use]
    pub const fn depot_refit_cargo(self) -> Option<CargoType> {
        match self {
            Self::Depot { refit_cargo, .. } => refit_cargo,
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_pass_through(self) -> bool {
        matches!(
            self,
            Self::Waypoint { .. } | Self::Depot { stop: false, .. }
        )
    }

    #[must_use]
    pub const fn full_load(self) -> bool {
        matches!(
            self,
            Self::Station {
                full_load: true,
                ..
            }
        )
    }

    #[must_use]
    pub const fn no_unload(self) -> bool {
        matches!(
            self,
            Self::Station {
                no_unload: true,
                ..
            }
        )
    }

    /// Trasbordo forzado: no cobra entrega, solo acumula `feeder_share`.
    #[must_use]
    pub const fn transfer(self) -> bool {
        matches!(
            self,
            Self::Station {
                transfer: true,
                ..
            }
        )
    }

    /// Alterna «carga completa» en una parada de estación.
    #[must_use]
    pub fn with_toggled_full_load(self) -> Option<Self> {
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks,
                travel_ticks,
            } => Some(Self::Station {
                station,
                full_load: !full_load,
                no_unload,
                transfer,
                wait_ticks,
                travel_ticks,
            }),
            _ => None,
        }
    }

    /// Alterna «no descargar» en una parada de estación.
    #[must_use]
    pub fn with_toggled_no_unload(self) -> Option<Self> {
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks,
                travel_ticks,
            } => Some(Self::Station {
                station,
                full_load,
                no_unload: !no_unload,
                transfer,
                wait_ticks,
                travel_ticks,
            }),
            _ => None,
        }
    }

    /// Alterna «parar en depósito» en una orden de depósito.
    #[must_use]
    pub fn with_toggled_depot_stop(self) -> Option<Self> {
        match self {
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
            } => Some(Self::Depot {
                depot,
                stop: !stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
            }),
            _ => None,
        }
    }

    /// Cicla el tipo de refit de una orden de depósito (`None` → opciones → `None`).
    #[must_use]
    pub fn with_cycled_depot_refit(self, options: &[CargoType]) -> Option<Self> {
        match self {
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
            } => {
                let next = match refit_cargo {
                    None => options.first().copied(),
                    Some(current) => {
                        let idx = options.iter().position(|&c| c == current)?;
                        if idx + 1 < options.len() {
                            Some(options[idx + 1])
                        } else {
                            None
                        }
                    }
                };
                Some(Self::Depot {
                    depot,
                    stop,
                    wait_ticks,
                    travel_ticks,
                    refit_cargo: next,
                })
            }
            _ => None,
        }
    }

    #[must_use]
    pub const fn wait_ticks(self) -> u32 {
        match self {
            Self::Station { wait_ticks, .. } | Self::Depot { wait_ticks, .. } => wait_ticks,
            _ => 0,
        }
    }

    #[must_use]
    pub const fn travel_ticks(self) -> u32 {
        match self {
            Self::Station { travel_ticks, .. }
            | Self::Waypoint { travel_ticks, .. }
            | Self::Depot { travel_ticks, .. } => travel_ticks,
            Self::Tile(_) | Self::Conditional { .. } => 0,
        }
    }

    #[must_use]
    pub fn with_cycled_wait(self) -> Option<Self> {
        use crate::timetable::cycle_wait_ticks;
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks,
                travel_ticks,
            } => Some(Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks: cycle_wait_ticks(wait_ticks),
                travel_ticks,
            }),
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
            } => Some(Self::Depot {
                depot,
                stop,
                wait_ticks: cycle_wait_ticks(wait_ticks),
                travel_ticks,
                refit_cargo,
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn with_cycled_travel(self) -> Self {
        use crate::timetable::cycle_travel_ticks;
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks,
                travel_ticks,
            } => Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks,
                travel_ticks: cycle_travel_ticks(travel_ticks),
            },
            Self::Waypoint {
                waypoint,
                travel_ticks,
            } => Self::Waypoint {
                waypoint,
                travel_ticks: cycle_travel_ticks(travel_ticks),
            },
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
            } => Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks: cycle_travel_ticks(travel_ticks),
                refit_cargo,
            },
            Self::Tile(t) => Self::Tile(t),
            Self::Conditional {
                condition,
                value,
                jump_to,
            } => Self::Conditional {
                condition,
                value,
                jump_to,
            },
        }
    }

    #[must_use]
    pub fn with_wait_ticks(self, wait_ticks: u32) -> Option<Self> {
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                travel_ticks,
                ..
            } => Some(Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks,
                travel_ticks,
            }),
            Self::Depot {
                depot,
                stop,
                travel_ticks,
                refit_cargo,
                ..
            } => Some(Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn with_travel_ticks(self, travel_ticks: u32) -> Self {
        match self {
            Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks,
                ..
            } => Self::Station {
                station,
                full_load,
                no_unload,
                transfer,
                wait_ticks,
                travel_ticks,
            },
            Self::Waypoint { waypoint, .. } => Self::Waypoint {
                waypoint,
                travel_ticks,
            },
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                refit_cargo,
                ..
            } => Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
            },
            other => other,
        }
    }
}
