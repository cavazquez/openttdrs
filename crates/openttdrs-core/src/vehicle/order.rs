//! Órdenes de vehículos: estaciones, waypoints, depósitos, condicionales.

use crate::cargo::CargoType;
use crate::map::TileCoord;

/// Nombre personalizado del jugador (`OpenTTD` `MAX_LENGTH_VEHICLE_NAME_CHARS` = 32).
pub const MAX_VEHICLE_NAME_CHARS: usize = 32;

/// Paradas intermedias en ruta (`OrderNonStopFlag` en bits 6–7 de `Order::type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderNonStop {
    /// Parar en estaciones intermedias del trayecto (`GoVia`, bit 6 de `type`).
    StopAtIntermediate,
    /// No parar salvo en el destino de la orden (`NonStop`, default en `OpenTTD`).
    #[default]
    NonStopDestination,
}

/// Punto de parada en el andén (`OrderStopLocation`, bits 4–5 de `Order::type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStopLocation {
    /// Extremo cercano al sentido de entrada.
    NearEnd = 0,
    /// Centro del andén (default `OpenTTD` / port).
    #[default]
    Middle = 1,
    /// Extremo lejano del andén.
    FarEnd = 2,
}

impl OrderStopLocation {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x3 {
            0 => Self::NearEnd,
            2 => Self::FarEnd,
            _ => Self::Middle,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Cicla Near → Middle → Far → Near.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::NearEnd => Self::Middle,
            Self::Middle => Self::FarEnd,
            Self::FarEnd => Self::NearEnd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum VehicleOrder {
    Station {
        station: TileCoord,
        /// Esperar carga completa de todos los tipos (`OrderLoadType::FullLoad`).
        #[serde(default)]
        full_load: bool,
        /// Salir cuando cualquier tipo de carga esté lleno (`OrderLoadType::FullLoadAny`).
        #[serde(default)]
        full_load_any: bool,
        /// No cargar en esta parada (`OrderLoadType::NoLoad`).
        #[serde(default)]
        no_load: bool,
        /// No descargar en esta parada (`OrderUnloadType::NoUnload`).
        #[serde(default)]
        no_unload: bool,
        /// Trasbordo forzado (`OrderUnloadType::Transfer`): acumula feeder, no cobra.
        #[serde(default)]
        transfer: bool,
        /// Sin parar en estaciones intermedias hacia este destino.
        #[serde(default)]
        non_stop: OrderNonStop,
        /// Dónde detener el tren en el andén (`GetTrainStopLocation`).
        #[serde(default)]
        stop_location: OrderStopLocation,
        /// Espera mínima en parada con horario activo (ticks de sim).
        #[serde(default)]
        wait_ticks: u32,
        /// Tiempo mínimo de viaje hasta esta orden (ticks desde la anterior).
        #[serde(default)]
        travel_ticks: u32,
        /// Orden implícita (`OT_IMPLICIT`) insertada al visitar una estación.
        #[serde(default)]
        implicit: bool,
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
            full_load_any: false,
            no_load: false,
            no_unload: false,
            transfer: false,
            non_stop: OrderNonStop::NonStopDestination,
            stop_location: OrderStopLocation::Middle,
            wait_ticks: 0,
            travel_ticks: 0,
            implicit: false,
        }
    }

    /// Orden implícita (`OT_IMPLICIT`) al visitar una estación no programada.
    #[must_use]
    pub const fn implicit(station: TileCoord) -> Self {
        Self::Station {
            station,
            full_load: false,
            full_load_any: false,
            no_load: false,
            no_unload: false,
            transfer: false,
            non_stop: OrderNonStop::NonStopDestination,
            stop_location: OrderStopLocation::Middle,
            wait_ticks: 0,
            travel_ticks: 0,
            implicit: true,
        }
    }

    /// Siguiente estación distinta en la lista circular (`CargoDist` Manual / `next_hop`).
    ///
    /// Delega en [`Self::get_next_stopping_station`] (P2.22).
    #[must_use]
    pub fn next_station_hop(
        orders: &[Self],
        current_order: usize,
        from: TileCoord,
    ) -> Option<TileCoord> {
        Self::get_next_stopping_station(orders, current_order, from, None)
            .into_iter()
            .next()
    }

    /// `OrderList::GetNextStoppingStation` — recorrido recursivo con vías y condicionales.
    ///
    /// Devuelve una o más estaciones candidatas (ambas ramas de un condicional).
    #[must_use]
    pub fn get_next_stopping_station(
        orders: &[Self],
        cur_implicit: usize,
        last_station: TileCoord,
        cargo_load_pct: Option<u8>,
    ) -> Vec<TileCoord> {
        let mut out = Vec::new();
        Self::collect_next_stopping_station(
            orders,
            cur_implicit,
            last_station,
            cargo_load_pct,
            None,
            0,
            &mut out,
        );
        out
    }

    #[allow(clippy::too_many_lines)]
    fn collect_next_stopping_station(
        orders: &[Self],
        cur_implicit: usize,
        last_station: TileCoord,
        cargo_load_pct: Option<u8>,
        first: Option<usize>,
        hops: usize,
        out: &mut Vec<TileCoord>,
    ) {
        if orders.is_empty() || hops > orders.len().saturating_mul(2) {
            return;
        }
        let n = orders.len();
        let mut next = if let Some(idx) = first {
            idx % n
        } else {
            let start = cur_implicit.min(n.saturating_sub(1));
            (start + 1) % n
        };
        let origin = first.unwrap_or(cur_implicit.min(n.saturating_sub(1)));

        loop {
            // Resolver condicionales (ambas ramas si hace falta).
            while matches!(orders.get(next), Some(Self::Conditional { .. })) {
                let Some(Self::Conditional {
                    condition,
                    value,
                    jump_to,
                }) = orders.get(next).copied()
                else {
                    break;
                };
                let pct = cargo_load_pct.unwrap_or(0);
                let take_jump = match condition {
                    OrderConditionKind::CargoLoadAbove => pct > value,
                    OrderConditionKind::CargoLoadBelow => pct < value,
                };
                // Sin porcentaje conocido: explorar ambas ramas (estimación OpenTTD).
                if cargo_load_pct.is_none() {
                    let skip_to = jump_to.min(n.saturating_sub(1));
                    let advance = (next + 1) % n;
                    if skip_to != origin {
                        Self::collect_next_stopping_station(
                            orders,
                            cur_implicit,
                            last_station,
                            cargo_load_pct,
                            Some(skip_to),
                            hops + 1,
                            out,
                        );
                    }
                    if advance != origin && advance != skip_to {
                        Self::collect_next_stopping_station(
                            orders,
                            cur_implicit,
                            last_station,
                            cargo_load_pct,
                            Some(advance),
                            hops + 1,
                            out,
                        );
                    }
                    return;
                }
                next = if take_jump {
                    jump_to.min(n.saturating_sub(1))
                } else {
                    (next + 1) % n
                };
            }

            let Some(order) = orders.get(next) else {
                return;
            };
            match order {
                Self::Station {
                    station,
                    transfer,
                    non_stop,
                    ..
                } if *station != last_station => {
                    let _ = (transfer, non_stop);
                    if !out.contains(station) {
                        out.push(*station);
                    }
                    return;
                }
                Self::Station {
                    station,
                    transfer: true,
                    ..
                } if *station == last_station => {
                    return;
                }
                Self::Depot { .. }
                | Self::Station { .. }
                | Self::Waypoint { .. }
                | Self::Tile(_)
                | Self::Conditional { .. } => {
                    next = (next + 1) % n;
                    if next == origin {
                        return;
                    }
                }
            }
        }
    }

    #[must_use]
    pub const fn is_implicit(self) -> bool {
        matches!(self, Self::Station { implicit: true, .. })
    }

    #[must_use]
    pub const fn is_station_like(self) -> bool {
        matches!(self, Self::Station { .. })
    }

    #[must_use]
    pub const fn station_with_flags(station: TileCoord, full_load: bool, no_unload: bool) -> Self {
        Self::Station {
            station,
            full_load,
            full_load_any: false,
            no_load: false,
            no_unload,
            transfer: false,
            non_stop: OrderNonStop::NonStopDestination,
            stop_location: OrderStopLocation::Middle,
            wait_ticks: 0,
            travel_ticks: 0,
            implicit: false,
        }
    }

    /// Construye una parada con todos los flags de carga/descarga/ruta.
    #[must_use]
    #[allow(clippy::fn_params_excessive_bools)]
    pub const fn station_with_load_unload_flags(
        station: TileCoord,
        full_load: bool,
        full_load_any: bool,
        no_load: bool,
        no_unload: bool,
        transfer: bool,
        non_stop: OrderNonStop,
    ) -> Self {
        Self::Station {
            station,
            full_load,
            full_load_any,
            no_load,
            no_unload,
            transfer,
            non_stop,
            stop_location: OrderStopLocation::Middle,
            wait_ticks: 0,
            travel_ticks: 0,
            implicit: false,
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
    pub const fn full_load_any(self) -> bool {
        matches!(
            self,
            Self::Station {
                full_load_any: true,
                ..
            }
        )
    }

    /// `FullLoad` o `FullLoadAny` (`Order::IsFullLoadOrder`).
    #[must_use]
    pub const fn is_full_load_order(self) -> bool {
        self.full_load() || self.full_load_any()
    }

    #[must_use]
    pub const fn no_load(self) -> bool {
        matches!(self, Self::Station { no_load: true, .. })
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

    #[must_use]
    pub const fn non_stop_destination(self) -> bool {
        matches!(
            self,
            Self::Station {
                non_stop: OrderNonStop::NonStopDestination,
                ..
            }
        )
    }

    #[must_use]
    pub const fn stop_location(self) -> OrderStopLocation {
        match self {
            Self::Station { stop_location, .. } => stop_location,
            _ => OrderStopLocation::Middle,
        }
    }

    /// Cicla Near → Middle → Far en una orden de estación.
    #[must_use]
    pub fn with_cycled_stop_location(self) -> Option<Self> {
        match self {
            Self::Station {
                station,
                full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
            } => Some(Self::Station {
                station,
                full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location: stop_location.next(),
                wait_ticks,
                travel_ticks,
                implicit,
            }),
            _ => None,
        }
    }

    /// ¿Debe seguir esperando carga según `FullLoad` / `FullLoadAny`?
    #[must_use]
    pub const fn should_wait_for_loading(self, cargo: u32, capacity: u32) -> bool {
        if self.no_load() || capacity == 0 {
            return false;
        }
        if self.full_load() {
            return cargo < capacity;
        }
        if self.full_load_any() {
            return cargo < capacity;
        }
        false
    }

    /// Trasbordo forzado: no cobra entrega, solo acumula `feeder_share`.
    #[must_use]
    pub const fn transfer(self) -> bool {
        matches!(self, Self::Station { transfer: true, .. })
    }

    /// Alterna «carga completa» en una parada de estación.
    #[must_use]
    pub fn with_toggled_full_load(self) -> Option<Self> {
        match self {
            Self::Station {
                station,
                full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
            } => Some(Self::Station {
                station,
                full_load: !full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
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
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
            } => Some(Self::Station {
                station,
                full_load,
                full_load_any,
                no_load,
                no_unload: !no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
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
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
            } => Some(Self::Station {
                station,
                full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks: cycle_wait_ticks(wait_ticks),
                travel_ticks,
                implicit,
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
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
            } => Self::Station {
                station,
                full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks: cycle_travel_ticks(travel_ticks),
                implicit,
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
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks: _,
                travel_ticks,
                implicit,
            } => Some(Self::Station {
                station,
                full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
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
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks: _,
                implicit,
            } => Self::Station {
                station,
                full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                implicit,
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
