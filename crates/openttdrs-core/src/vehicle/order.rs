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

/// Política de carga de una orden de estación (`OrderLoadType` de `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderLoadType {
    /// Cargar lo disponible y salir cuando termina la transferencia del tick.
    #[default]
    LoadIfPossible,
    /// Esperar hasta completar todas las capacidades del consist.
    FullLoad,
    /// Esperar hasta completar al menos un tipo de carga del consist.
    FullLoadAny,
    /// No cargar en esta parada.
    NoLoad,
}

impl OrderLoadType {
    /// Orden de ciclo que usa el botón de la ventana de órdenes.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::LoadIfPossible => Self::FullLoad,
            Self::FullLoad => Self::FullLoadAny,
            Self::FullLoadAny => Self::NoLoad,
            Self::NoLoad => Self::LoadIfPossible,
        }
    }
}

/// Política de descarga de una orden de estación (`OrderUnloadType` de `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderUnloadType {
    /// Descargar únicamente la carga aceptada o encaminada a esta estación.
    #[default]
    UnloadIfPossible,
    /// Forzar la descarga: entregar si se acepta y transferir en caso contrario.
    Unload,
    /// Transferir siempre, sin cobro de entrega final.
    Transfer,
    /// Mantener toda la carga a bordo.
    NoUnload,
}

impl OrderUnloadType {
    /// Orden de ciclo que usa el botón de la ventana de órdenes.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::UnloadIfPossible => Self::Unload,
            Self::Unload => Self::Transfer,
            Self::Transfer => Self::NoUnload,
            Self::NoUnload => Self::UnloadIfPossible,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VehicleOrder {
    Station {
        station: TileCoord,
        /// Política completa de carga de `OpenTTD`.
        load_type: OrderLoadType,
        /// Política completa de descarga de `OpenTTD`.
        unload_type: OrderUnloadType,
        /// Sin parar en estaciones intermedias hacia este destino.
        non_stop: OrderNonStop,
        /// Dónde detener el tren en el andén (`GetTrainStopLocation`).
        stop_location: OrderStopLocation,
        /// Espera mínima en parada con horario activo (ticks de sim).
        wait_ticks: u32,
        /// Tiempo mínimo de viaje hasta esta orden (ticks desde la anterior).
        travel_ticks: u32,
        /// Techo de velocidad del tramo (`Order::max_speed`); 0 = sin límite.
        max_speed: u16,
        /// Orden implícita (`OT_IMPLICIT`) insertada al visitar una estación.
        implicit: bool,
    },
    Waypoint {
        waypoint: TileCoord,
        travel_ticks: u32,
        /// Techo de velocidad del tramo; 0 = sin límite.
        max_speed: u16,
    },
    /// Parada en depósito (`stop`: detener al llegar; `false` = pasar sin parar).
    Depot {
        depot: TileCoord,
        stop: bool,
        wait_ticks: u32,
        travel_ticks: u32,
        /// Refit automático al llegar (solo si `stop` y sin carga a bordo).
        refit_cargo: Option<CargoType>,
        /// Servicio + unbunch (`OrderDepotActionFlag::Unbunch`).
        unbunch: bool,
    },
    Tile(TileCoord),
    /// Salta a `jump_to` si la condición se cumple al llegar a esta «orden».
    Conditional {
        condition: OrderConditionKind,
        comparator: OrderConditionComparator,
        value: u16,
        jump_to: usize,
    },
}

/// Representación serde compatible con los saves v24, que guardaban cinco
/// booleanos independientes. Los saves nuevos escriben solamente los dos enums.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum VehicleOrderSerde {
    Station {
        station: TileCoord,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        load_type: Option<OrderLoadType>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unload_type: Option<OrderUnloadType>,
        #[serde(default, skip_serializing_if = "is_false")]
        full_load: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        full_load_any: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        no_load: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        no_unload: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        transfer: bool,
        #[serde(default)]
        non_stop: OrderNonStop,
        #[serde(default)]
        stop_location: OrderStopLocation,
        #[serde(default)]
        wait_ticks: u32,
        #[serde(default)]
        travel_ticks: u32,
        #[serde(default)]
        max_speed: u16,
        #[serde(default)]
        implicit: bool,
    },
    Waypoint {
        waypoint: TileCoord,
        #[serde(default)]
        travel_ticks: u32,
        #[serde(default)]
        max_speed: u16,
    },
    Depot {
        depot: TileCoord,
        #[serde(default = "default_depot_stop")]
        stop: bool,
        #[serde(default)]
        wait_ticks: u32,
        #[serde(default)]
        travel_ticks: u32,
        #[serde(default)]
        refit_cargo: Option<CargoType>,
        #[serde(default)]
        unbunch: bool,
    },
    Tile(TileCoord),
    Conditional {
        condition: OrderConditionKind,
        #[serde(default)]
        comparator: Option<OrderConditionComparator>,
        value: u16,
        jump_to: usize,
    },
}

#[allow(clippy::trivially_copy_pass_by_ref)] // firma requerida por serde skip_serializing_if
const fn is_false(value: &bool) -> bool {
    !*value
}

impl From<VehicleOrder> for VehicleOrderSerde {
    fn from(order: VehicleOrder) -> Self {
        match order {
            VehicleOrder::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            } => Self::Station {
                station,
                load_type: Some(load_type),
                unload_type: Some(unload_type),
                full_load: false,
                full_load_any: false,
                no_load: false,
                no_unload: false,
                transfer: false,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            },
            VehicleOrder::Waypoint {
                waypoint,
                travel_ticks,
                max_speed,
            } => Self::Waypoint {
                waypoint,
                travel_ticks,
                max_speed,
            },
            VehicleOrder::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            } => Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            },
            VehicleOrder::Tile(tile) => Self::Tile(tile),
            VehicleOrder::Conditional {
                condition,
                comparator,
                value,
                jump_to,
            } => Self::Conditional {
                condition,
                comparator: Some(comparator),
                value,
                jump_to,
            },
        }
    }
}

impl From<VehicleOrderSerde> for VehicleOrder {
    fn from(order: VehicleOrderSerde) -> Self {
        match order {
            VehicleOrderSerde::Station {
                station,
                load_type,
                unload_type,
                full_load,
                full_load_any,
                no_load,
                no_unload,
                transfer,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            } => Self::Station {
                station,
                load_type: load_type.unwrap_or({
                    if no_load {
                        OrderLoadType::NoLoad
                    } else if full_load_any {
                        OrderLoadType::FullLoadAny
                    } else if full_load {
                        OrderLoadType::FullLoad
                    } else {
                        OrderLoadType::LoadIfPossible
                    }
                }),
                unload_type: unload_type.unwrap_or({
                    if transfer {
                        OrderUnloadType::Transfer
                    } else if no_unload {
                        OrderUnloadType::NoUnload
                    } else {
                        OrderUnloadType::UnloadIfPossible
                    }
                }),
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            },
            VehicleOrderSerde::Waypoint {
                waypoint,
                travel_ticks,
                max_speed,
            } => Self::Waypoint {
                waypoint,
                travel_ticks,
                max_speed,
            },
            VehicleOrderSerde::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            } => Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            },
            VehicleOrderSerde::Tile(tile) => Self::Tile(tile),
            VehicleOrderSerde::Conditional {
                condition,
                comparator,
                value,
                jump_to,
            } => Self::Conditional {
                condition,
                comparator: comparator
                    .or_else(|| condition.legacy_comparator())
                    .unwrap_or(OrderConditionComparator::MoreThan),
                value,
                jump_to,
            },
        }
    }
}

impl serde::Serialize for VehicleOrder {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&VehicleOrderSerde::from(*self), serializer)
    }
}

impl<'de> serde::Deserialize<'de> for VehicleOrder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <VehicleOrderSerde as serde::Deserialize>::deserialize(deserializer).map(Self::from)
    }
}

/// Comparador de orden condicional (`OrderConditionComparator` en `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderConditionComparator {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    #[default]
    MoreThan,
    MoreThanOrEqual,
    IsTrue,
    IsFalse,
}

impl OrderConditionComparator {
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Equal => 0,
            Self::NotEqual => 1,
            Self::LessThan => 2,
            Self::LessThanOrEqual => 3,
            Self::MoreThan => 4,
            Self::MoreThanOrEqual => 5,
            Self::IsTrue => 6,
            Self::IsFalse => 7,
        }
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Equal,
            1 => Self::NotEqual,
            2 => Self::LessThan,
            3 => Self::LessThanOrEqual,
            4 => Self::MoreThan,
            5 => Self::MoreThanOrEqual,
            6 => Self::IsTrue,
            _ => Self::IsFalse,
        }
    }

    #[must_use]
    pub fn compare(self, lhs: u16, rhs: u16) -> bool {
        match self {
            Self::Equal => lhs == rhs,
            Self::NotEqual => lhs != rhs,
            Self::LessThan => lhs < rhs,
            Self::LessThanOrEqual => lhs <= rhs,
            Self::MoreThan => lhs > rhs,
            Self::MoreThanOrEqual => lhs >= rhs,
            Self::IsTrue => lhs != 0,
            Self::IsFalse => lhs == 0,
        }
    }
}

/// Variable de orden condicional (`OrderConditionVariable` en `OpenTTD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderConditionKind {
    /// Alias histórico → `LoadPercentage` + `MoreThan`.
    CargoLoadAbove,
    /// Alias histórico → `LoadPercentage` + `LessThan`.
    CargoLoadBelow,
    LoadPercentage,
    Reliability,
    MaxSpeed,
    Age,
    RequiresService,
    Unconditionally,
    RemainingLifetime,
    MaxReliability,
    DrivingBackwards,
}

impl OrderConditionKind {
    /// Comparador implícito de los alias históricos (si no se serializó otro).
    #[must_use]
    pub const fn legacy_comparator(self) -> Option<OrderConditionComparator> {
        match self {
            Self::CargoLoadAbove => Some(OrderConditionComparator::MoreThan),
            Self::CargoLoadBelow => Some(OrderConditionComparator::LessThan),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::CargoLoadAbove | Self::LoadPercentage | Self::CargoLoadBelow => 0,
            Self::Reliability => 1,
            Self::MaxSpeed => 2,
            Self::Age => 3,
            Self::RequiresService => 4,
            Self::Unconditionally => 5,
            Self::RemainingLifetime => 6,
            Self::MaxReliability => 7,
            Self::DrivingBackwards => 8,
        }
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Reliability,
            2 => Self::MaxSpeed,
            3 => Self::Age,
            4 => Self::RequiresService,
            5 => Self::Unconditionally,
            6 => Self::RemainingLifetime,
            7 => Self::MaxReliability,
            8 => Self::DrivingBackwards,
            _ => Self::LoadPercentage,
        }
    }
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
    pub const fn conditional(condition: OrderConditionKind, value: u16, jump_to: usize) -> Self {
        let comparator = match condition.legacy_comparator() {
            Some(c) => c,
            None => OrderConditionComparator::MoreThan,
        };
        Self::Conditional {
            condition,
            comparator,
            value,
            jump_to,
        }
    }

    #[must_use]
    pub const fn conditional_with(
        condition: OrderConditionKind,
        comparator: OrderConditionComparator,
        value: u16,
        jump_to: usize,
    ) -> Self {
        Self::Conditional {
            condition,
            comparator,
            value,
            jump_to,
        }
    }

    #[must_use]
    pub const fn is_conditional(self) -> bool {
        matches!(self, Self::Conditional { .. })
    }

    /// Valor de la variable condicional para un vehículo.
    #[must_use]
    pub fn condition_value(condition: OrderConditionKind, vehicle: &super::model::Vehicle) -> u16 {
        match condition {
            OrderConditionKind::CargoLoadAbove
            | OrderConditionKind::CargoLoadBelow
            | OrderConditionKind::LoadPercentage => {
                if vehicle.capacity == 0 {
                    0
                } else {
                    u16::try_from(
                        (u64::from(vehicle.cargo) * 100 / u64::from(vehicle.capacity)).min(100),
                    )
                    .unwrap_or(100)
                }
            }
            // OpenTTD compara fiabilidad en porcentaje (0..=100).
            OrderConditionKind::Reliability | OrderConditionKind::MaxReliability => {
                (vehicle.reliability / 100).min(100)
            }
            OrderConditionKind::MaxSpeed => {
                if vehicle.cached_max_speed == u16::MAX {
                    0
                } else {
                    vehicle.cached_max_speed
                }
            }
            OrderConditionKind::Age => {
                let days = vehicle.vehicle_age_days(vehicle.sim_tick);
                u16::try_from(days.min(u64::from(u16::MAX))).unwrap_or(u16::MAX)
            }
            OrderConditionKind::RequiresService => u16::from(vehicle.needs_servicing),
            OrderConditionKind::Unconditionally => 1,
            OrderConditionKind::RemainingLifetime => {
                let age = vehicle.vehicle_age_days(vehicle.sim_tick);
                let left = u64::from(vehicle.max_age_days).saturating_sub(age);
                u16::try_from(left.min(u64::from(u16::MAX))).unwrap_or(u16::MAX)
            }
            OrderConditionKind::DrivingBackwards => 0,
        }
    }

    #[must_use]
    pub fn evaluate_conditional(self, vehicle: &super::model::Vehicle) -> usize {
        let Self::Conditional {
            condition,
            comparator,
            value,
            jump_to,
        } = self
        else {
            return vehicle.current_order;
        };
        let lhs = Self::condition_value(condition, vehicle);
        let comparator = condition.legacy_comparator().unwrap_or(comparator);
        let ok = match condition {
            OrderConditionKind::Unconditionally => true,
            OrderConditionKind::RequiresService | OrderConditionKind::DrivingBackwards => {
                match comparator {
                    OrderConditionComparator::IsFalse => lhs == 0,
                    OrderConditionComparator::Equal => lhs == value.min(1),
                    OrderConditionComparator::NotEqual => lhs != value.min(1),
                    _ => lhs != 0,
                }
            }
            _ => comparator.compare(lhs, value),
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
            load_type: OrderLoadType::LoadIfPossible,
            unload_type: OrderUnloadType::UnloadIfPossible,
            non_stop: OrderNonStop::NonStopDestination,
            stop_location: OrderStopLocation::Middle,
            wait_ticks: 0,
            travel_ticks: 0,
            max_speed: 0,
            implicit: false,
        }
    }

    /// Orden implícita (`OT_IMPLICIT`) al visitar una estación no programada.
    #[must_use]
    pub const fn implicit(station: TileCoord) -> Self {
        Self::Station {
            station,
            load_type: OrderLoadType::LoadIfPossible,
            unload_type: OrderUnloadType::UnloadIfPossible,
            non_stop: OrderNonStop::NonStopDestination,
            stop_location: OrderStopLocation::Middle,
            wait_ticks: 0,
            travel_ticks: 0,
            max_speed: 0,
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
                    comparator,
                    value,
                    jump_to,
                }) = orders.get(next).copied()
                else {
                    break;
                };
                let pct = u16::from(cargo_load_pct.unwrap_or(0));
                let comparator = condition.legacy_comparator().unwrap_or(comparator);
                let take_jump = match condition {
                    OrderConditionKind::Unconditionally => true,
                    OrderConditionKind::CargoLoadAbove
                    | OrderConditionKind::CargoLoadBelow
                    | OrderConditionKind::LoadPercentage => comparator.compare(pct, value),
                    _ => false,
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
                Self::Station { station, .. } if *station != last_station => {
                    if !out.contains(station) {
                        out.push(*station);
                    }
                    return;
                }
                Self::Station {
                    station,
                    unload_type: OrderUnloadType::Transfer,
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
            load_type: if full_load {
                OrderLoadType::FullLoad
            } else {
                OrderLoadType::LoadIfPossible
            },
            unload_type: if no_unload {
                OrderUnloadType::NoUnload
            } else {
                OrderUnloadType::UnloadIfPossible
            },
            non_stop: OrderNonStop::NonStopDestination,
            stop_location: OrderStopLocation::Middle,
            wait_ticks: 0,
            travel_ticks: 0,
            max_speed: 0,
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
        let load_type = if no_load {
            OrderLoadType::NoLoad
        } else if full_load_any {
            OrderLoadType::FullLoadAny
        } else if full_load {
            OrderLoadType::FullLoad
        } else {
            OrderLoadType::LoadIfPossible
        };
        let unload_type = if transfer {
            OrderUnloadType::Transfer
        } else if no_unload {
            OrderUnloadType::NoUnload
        } else {
            OrderUnloadType::UnloadIfPossible
        };
        Self::station_with_types(station, load_type, unload_type, non_stop)
    }

    /// Construye una parada con las políticas completas de carga y descarga.
    #[must_use]
    pub const fn station_with_types(
        station: TileCoord,
        load_type: OrderLoadType,
        unload_type: OrderUnloadType,
        non_stop: OrderNonStop,
    ) -> Self {
        Self::Station {
            station,
            load_type,
            unload_type,
            non_stop,
            stop_location: OrderStopLocation::Middle,
            wait_ticks: 0,
            travel_ticks: 0,
            max_speed: 0,
            implicit: false,
        }
    }

    #[must_use]
    pub const fn waypoint(waypoint: TileCoord) -> Self {
        Self::Waypoint {
            waypoint,
            travel_ticks: 0,
            max_speed: 0,
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
            unbunch: false,
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
            unbunch: false,
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
    pub const fn load_type(self) -> OrderLoadType {
        match self {
            Self::Station { load_type, .. } => load_type,
            _ => OrderLoadType::LoadIfPossible,
        }
    }

    #[must_use]
    pub const fn full_load(self) -> bool {
        matches!(
            self,
            Self::Station {
                load_type: OrderLoadType::FullLoad,
                ..
            }
        )
    }

    #[must_use]
    pub const fn full_load_any(self) -> bool {
        matches!(
            self,
            Self::Station {
                load_type: OrderLoadType::FullLoadAny,
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
        matches!(
            self,
            Self::Station {
                load_type: OrderLoadType::NoLoad,
                ..
            }
        )
    }

    #[must_use]
    pub const fn unload_type(self) -> OrderUnloadType {
        match self {
            Self::Station { unload_type, .. } => unload_type,
            _ => OrderUnloadType::UnloadIfPossible,
        }
    }

    #[must_use]
    pub const fn no_unload(self) -> bool {
        matches!(
            self,
            Self::Station {
                unload_type: OrderUnloadType::NoUnload,
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

    /// Equivalente a `Order::ShouldStopAtStation` de OpenTTD 15.3.
    #[must_use]
    pub const fn should_stop_at_station(
        self,
        last_station_visited: Option<TileCoord>,
        station: TileCoord,
    ) -> bool {
        if matches!(last_station_visited, Some(last) if last.x == station.x && last.y == station.y) {
            return false;
        }
        match self {
            Self::Station {
                station: destination,
                non_stop,
                ..
            } => {
                (destination.x == station.x && destination.y == station.y)
                    || matches!(non_stop, OrderNonStop::StopAtIntermediate)
            }
            _ => false,
        }
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
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            } => Some(Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location: stop_location.next(),
                wait_ticks,
                travel_ticks,
                max_speed,
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

    /// Decide la espera de carga sobre todas las unidades de un consist.
    ///
    /// Cada tupla es `(tipo, carga, capacidad)`. `FullLoad` espera por toda
    /// unidad con capacidad; `FullLoadAny` termina cuando un tipo de carga está
    /// completo en el conjunto de unidades que transportan ese tipo.
    #[must_use]
    pub fn should_wait_for_consist_loading(self, units: &[(CargoType, u32, u32)]) -> bool {
        match self.load_type() {
            OrderLoadType::LoadIfPossible | OrderLoadType::NoLoad => false,
            OrderLoadType::FullLoad => units
                .iter()
                .any(|(_, cargo, capacity)| *capacity > 0 && cargo < capacity),
            OrderLoadType::FullLoadAny => {
                let has_capacity = units.iter().any(|(_, _, capacity)| *capacity > 0);
                has_capacity
                    && !crate::cargo::ALL_CARGO_TYPES.iter().any(|cargo_type| {
                        let (cargo, capacity) = units
                            .iter()
                            .filter(|(kind, _, capacity)| kind == cargo_type && *capacity > 0)
                            .fold((0_u64, 0_u64), |(cargo_sum, capacity_sum), (_, c, cap)| {
                                (cargo_sum + u64::from(*c), capacity_sum + u64::from(*cap))
                            });
                        capacity > 0 && cargo >= capacity
                    })
            }
        }
    }

    /// Trasbordo forzado: no cobra entrega, solo acumula `feeder_share`.
    #[must_use]
    pub const fn transfer(self) -> bool {
        matches!(
            self,
            Self::Station {
                unload_type: OrderUnloadType::Transfer,
                ..
            }
        )
    }

    /// Cicla los cuatro tipos de carga en una parada de estación.
    #[must_use]
    pub fn with_toggled_full_load(self) -> Option<Self> {
        match self {
            Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            } => Some(Self::Station {
                station,
                load_type: load_type.next(),
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            }),
            _ => None,
        }
    }

    /// Cicla los cuatro tipos de descarga en una parada de estación.
    #[must_use]
    pub fn with_toggled_no_unload(self) -> Option<Self> {
        match self {
            Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            } => Some(Self::Station {
                station,
                load_type,
                unload_type: unload_type.next(),
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
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
                unbunch,
            } => Some(Self::Depot {
                depot,
                stop: !stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
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
                unbunch,
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
                    unbunch,
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
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            } => Some(Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks: cycle_wait_ticks(wait_ticks),
                travel_ticks,
                max_speed,
                implicit,
            }),
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            } => Some(Self::Depot {
                depot,
                stop,
                wait_ticks: cycle_wait_ticks(wait_ticks),
                travel_ticks,
                refit_cargo,
                unbunch,
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
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            } => Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks: cycle_travel_ticks(travel_ticks),
                max_speed,
                implicit,
            },
            Self::Waypoint {
                waypoint,
                travel_ticks,
                max_speed,
            } => Self::Waypoint {
                waypoint,
                travel_ticks: cycle_travel_ticks(travel_ticks),
                max_speed,
            },
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            } => Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks: cycle_travel_ticks(travel_ticks),
                refit_cargo,
                unbunch,
            },
            Self::Tile(t) => Self::Tile(t),
            Self::Conditional {
                condition,
                comparator,
                value,
                jump_to,
            } => Self::Conditional {
                condition,
                comparator,
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
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks: _,
                travel_ticks,
                max_speed,
                implicit,
            } => Some(Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            }),
            Self::Depot {
                depot,
                stop,
                travel_ticks,
                refit_cargo,
                unbunch,
                ..
            } => Some(Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            }),
            _ => None,
        }
    }

    #[must_use]
    pub fn with_travel_ticks(self, travel_ticks: u32) -> Self {
        match self {
            Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks: _,
                max_speed,
                implicit,
            } => Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            },
            Self::Waypoint {
                waypoint,
                max_speed,
                ..
            } => Self::Waypoint {
                waypoint,
                travel_ticks,
                max_speed,
            },
            Self::Depot {
                depot,
                stop,
                wait_ticks,
                refit_cargo,
                unbunch,
                ..
            } => Self::Depot {
                depot,
                stop,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            },
            other => other,
        }
    }

    #[must_use]
    pub const fn max_speed_limit(self) -> u16 {
        match self {
            Self::Station { max_speed, .. } | Self::Waypoint { max_speed, .. } => max_speed,
            _ => 0,
        }
    }

    #[must_use]
    pub fn with_max_speed(self, max_speed: u16) -> Self {
        match self {
            Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed: _,
                implicit,
            } => Self::Station {
                station,
                load_type,
                unload_type,
                non_stop,
                stop_location,
                wait_ticks,
                travel_ticks,
                max_speed,
                implicit,
            },
            Self::Waypoint {
                waypoint,
                travel_ticks,
                max_speed: _,
            } => Self::Waypoint {
                waypoint,
                travel_ticks,
                max_speed,
            },
            other => other,
        }
    }

    #[must_use]
    pub const fn depot_unbunch(self) -> bool {
        matches!(self, Self::Depot { unbunch: true, .. })
    }

    #[must_use]
    pub fn with_toggled_depot_unbunch(self) -> Option<Self> {
        match self {
            Self::Depot {
                depot,
                stop: _,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch,
            } => Some(Self::Depot {
                depot,
                // Unbunch implica servicio (no halt permanente).
                stop: true,
                wait_ticks,
                travel_ticks,
                refit_cargo,
                unbunch: !unbunch,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::vehicle::model::{Vehicle, VehicleKind};

    fn sample_vehicle() -> Vehicle {
        Vehicle::new(
            1,
            VehicleKind::Train,
            TileCoord::new(0, 0),
            TileCoord::new(1, 1),
        )
    }

    #[test]
    fn conditional_load_percentage_jumps() {
        let mut v = sample_vehicle();
        v.capacity = 100;
        v.cargo = 60;
        v.orders = vec![
            VehicleOrder::station(TileCoord::new(1, 1)),
            VehicleOrder::conditional(OrderConditionKind::LoadPercentage, 50, 0),
            VehicleOrder::station(TileCoord::new(2, 2)),
        ];
        v.current_order = 1;
        let next = v.orders[1].evaluate_conditional(&v);
        assert_eq!(next, 0);
        v.cargo = 10;
        let next = v.orders[1].evaluate_conditional(&v);
        assert_eq!(next, 2);
    }

    #[test]
    fn depot_unbunch_toggle_and_max_speed() {
        let depot = VehicleOrder::depot(TileCoord::new(3, 3));
        let toggled = depot.with_toggled_depot_unbunch().unwrap();
        assert!(toggled.depot_unbunch());
        assert!(toggled.depot_stops());
        let station = VehicleOrder::station(TileCoord::new(1, 1)).with_max_speed(80);
        assert_eq!(station.max_speed_limit(), 80);
    }

    #[test]
    fn should_stop_at_destination_unless_just_visited() {
        let station = TileCoord::new(3, 4);
        let order = VehicleOrder::station(station);
        assert!(order.should_stop_at_station(None, station));
        assert!(!order.should_stop_at_station(Some(station), station));
    }

    #[test]
    fn should_stop_at_intermediate_respects_non_stop_mode() {
        let destination = TileCoord::new(8, 4);
        let intermediate = TileCoord::new(3, 4);
        let non_stop = VehicleOrder::station(destination);
        let stopping = VehicleOrder::station_with_types(
            destination,
            OrderLoadType::LoadIfPossible,
            OrderUnloadType::UnloadIfPossible,
            OrderNonStop::StopAtIntermediate,
        );
        assert!(!non_stop.should_stop_at_station(None, intermediate));
        assert!(stopping.should_stop_at_station(None, intermediate));
    }

    #[test]
    fn reliability_and_unconditional_conditions() {
        let mut v = sample_vehicle();
        v.reliability = 7500; // 75%
        v.orders = vec![
            VehicleOrder::conditional_with(
                OrderConditionKind::Reliability,
                OrderConditionComparator::LessThan,
                80,
                0,
            ),
            VehicleOrder::station(TileCoord::new(1, 1)),
        ];
        v.current_order = 0;
        assert_eq!(v.orders[0].evaluate_conditional(&v), 0);
        v.orders[0] = VehicleOrder::conditional_with(
            OrderConditionKind::Unconditionally,
            OrderConditionComparator::IsTrue,
            0,
            0,
        );
        assert_eq!(v.orders[0].evaluate_conditional(&v), 0);
    }
}
