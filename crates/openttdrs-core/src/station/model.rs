use crate::cargo::{ALL_CARGO_TYPES, CargoStock, CargoType};
use crate::cargo_packet::StationCargoList;
use crate::company::CompanyId;
use crate::map::TileCoord;
use crate::vehicle::VehicleKind;
use serde::{Deserialize, Serialize};

/// Días desde la última recogida por tipo de carga (0 = reciente).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoTimeSincePickup {
    pub passengers: u8,
    pub coal: u8,
    pub mail: u8,
    pub oil: u8,
    pub livestock: u8,
    pub goods: u8,
    pub grain: u8,
    pub wood: u8,
    pub iron_ore: u8,
    pub steel: u8,
    pub valuables: u8,
    #[serde(default)]
    pub wheat: u8,
    #[serde(default)]
    pub paper: u8,
    #[serde(default)]
    pub gold: u8,
    #[serde(default)]
    pub food: u8,
    #[serde(default)]
    pub rubber: u8,
    #[serde(default)]
    pub fruit: u8,
    #[serde(default)]
    pub maize: u8,
    #[serde(default)]
    pub copper_ore: u8,
    #[serde(default)]
    pub water: u8,
    #[serde(default)]
    pub diamonds: u8,
    #[serde(default)]
    pub sugar: u8,
    #[serde(default)]
    pub toys: u8,
    #[serde(default)]
    pub batteries: u8,
    #[serde(default)]
    pub candy: u8,
    #[serde(default)]
    pub toffee: u8,
    #[serde(default)]
    pub cola: u8,
    #[serde(default)]
    pub cotton_candy: u8,
    #[serde(default)]
    pub bubbles: u8,
    #[serde(default)]
    pub plastic: u8,
    #[serde(default)]
    pub fizzy_drinks: u8,
    #[serde(
        default = "default_custom_time_since_pickup",
        serialize_with = "serialize_custom_time_since_pickup",
        deserialize_with = "deserialize_custom_time_since_pickup"
    )]
    pub custom: [u8; crate::cargo::CUSTOM_CARGO_COUNT],
}

impl Default for CargoTimeSincePickup {
    fn default() -> Self {
        Self {
            passengers: 0,
            coal: 0,
            mail: 0,
            oil: 0,
            livestock: 0,
            goods: 0,
            grain: 0,
            wood: 0,
            iron_ore: 0,
            steel: 0,
            valuables: 0,
            wheat: 0,
            paper: 0,
            gold: 0,
            food: 0,
            rubber: 0,
            fruit: 0,
            maize: 0,
            copper_ore: 0,
            water: 0,
            diamonds: 0,
            sugar: 0,
            toys: 0,
            batteries: 0,
            candy: 0,
            toffee: 0,
            cola: 0,
            cotton_candy: 0,
            bubbles: 0,
            plastic: 0,
            fizzy_drinks: 0,
            custom: [0; crate::cargo::CUSTOM_CARGO_COUNT],
        }
    }
}

fn default_custom_time_since_pickup() -> [u8; crate::cargo::CUSTOM_CARGO_COUNT] {
    [0; crate::cargo::CUSTOM_CARGO_COUNT]
}

fn serialize_custom_time_since_pickup<S>(
    custom: &[u8; crate::cargo::CUSTOM_CARGO_COUNT],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    custom.as_slice().serialize(serializer)
}

/// Acepta snapshots propios anteriores que sólo tenían 32 slots custom y
/// rellena el slot 63 con el valor por defecto.
fn deserialize_custom_time_since_pickup<'de, D>(
    deserializer: D,
) -> Result<[u8; crate::cargo::CUSTOM_CARGO_COUNT], D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values = Vec::<u8>::deserialize(deserializer)?;
    if values.len() > crate::cargo::CUSTOM_CARGO_COUNT {
        return Err(serde::de::Error::custom(format!(
            "CargoTimeSincePickup.custom: {} entradas > {}",
            values.len(),
            crate::cargo::CUSTOM_CARGO_COUNT
        )));
    }
    let mut custom = [0; crate::cargo::CUSTOM_CARGO_COUNT];
    custom[..values.len()].copy_from_slice(&values);
    Ok(custom)
}

impl CargoTimeSincePickup {
    #[must_use]
    pub const fn get(self, cargo: CargoType) -> u8 {
        match cargo {
            CargoType::Passengers => self.passengers,
            CargoType::Coal => self.coal,
            CargoType::Mail => self.mail,
            CargoType::Oil => self.oil,
            CargoType::Livestock => self.livestock,
            CargoType::Goods => self.goods,
            CargoType::Grain => self.grain,
            CargoType::Wood => self.wood,
            CargoType::IronOre => self.iron_ore,
            CargoType::Steel => self.steel,
            CargoType::Valuables => self.valuables,
            CargoType::Wheat => self.wheat,
            CargoType::Paper => self.paper,
            CargoType::Gold => self.gold,
            CargoType::Food => self.food,
            CargoType::Rubber => self.rubber,
            CargoType::Fruit => self.fruit,
            CargoType::Maize => self.maize,
            CargoType::CopperOre => self.copper_ore,
            CargoType::Water => self.water,
            CargoType::Diamonds => self.diamonds,
            CargoType::Sugar => self.sugar,
            CargoType::Toys => self.toys,
            CargoType::Batteries => self.batteries,
            CargoType::Candy => self.candy,
            CargoType::Toffee => self.toffee,
            CargoType::Cola => self.cola,
            CargoType::CottonCandy => self.cotton_candy,
            CargoType::Bubbles => self.bubbles,
            CargoType::Plastic => self.plastic,
            CargoType::FizzyDrinks => self.fizzy_drinks,
            CargoType::Custom(slot) => {
                let index = slot as usize;
                if index < crate::cargo::CUSTOM_CARGO_COUNT {
                    self.custom[index]
                } else {
                    0
                }
            }
        }
    }

    pub fn set(&mut self, cargo: CargoType, days: u8) {
        *self.slot_mut(cargo) = days;
    }

    pub fn increment_waiting(&mut self, cargo: CargoType) {
        let slot = self.slot_mut(cargo);
        *slot = slot.saturating_add(1);
    }

    fn slot_mut(&mut self, cargo: CargoType) -> &mut u8 {
        match cargo {
            CargoType::Passengers => &mut self.passengers,
            CargoType::Coal => &mut self.coal,
            CargoType::Mail => &mut self.mail,
            CargoType::Oil => &mut self.oil,
            CargoType::Livestock => &mut self.livestock,
            CargoType::Goods => &mut self.goods,
            CargoType::Grain => &mut self.grain,
            CargoType::Wood => &mut self.wood,
            CargoType::IronOre => &mut self.iron_ore,
            CargoType::Steel => &mut self.steel,
            CargoType::Valuables => &mut self.valuables,
            CargoType::Wheat => &mut self.wheat,
            CargoType::Paper => &mut self.paper,
            CargoType::Gold => &mut self.gold,
            CargoType::Food => &mut self.food,
            CargoType::Rubber => &mut self.rubber,
            CargoType::Fruit => &mut self.fruit,
            CargoType::Maize => &mut self.maize,
            CargoType::CopperOre => &mut self.copper_ore,
            CargoType::Water => &mut self.water,
            CargoType::Diamonds => &mut self.diamonds,
            CargoType::Sugar => &mut self.sugar,
            CargoType::Toys => &mut self.toys,
            CargoType::Batteries => &mut self.batteries,
            CargoType::Candy => &mut self.candy,
            CargoType::Toffee => &mut self.toffee,
            CargoType::Cola => &mut self.cola,
            CargoType::CottonCandy => &mut self.cotton_candy,
            CargoType::Bubbles => &mut self.bubbles,
            CargoType::Plastic => &mut self.plastic,
            CargoType::FizzyDrinks => &mut self.fizzy_drinks,
            CargoType::Custom(slot) => {
                &mut self.custom[usize::from(slot).min(crate::cargo::CUSTOM_CARGO_COUNT - 1)]
            }
        }
    }
}

/// Estado `NewGRF` que pertenece a una tesela de parada vial, no a toda la
/// estación lógica.
///
/// `OpenTTD` guarda una entrada `RoadStopTileData` por cada tesela custom de
/// una estación. Una parada compuesta puede mezclar specs, frames y random
/// bits; mantenerlos en la entidad `Station` completa congelaba o mezclaba
/// sus variantes después de `JoinStation`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct RoadStopTileState {
    /// Spec `NewGRF` aplicado a esta tesela (`None` = gráficos vanilla).
    #[serde(default)]
    pub spec: Option<u16>,
    /// Frame `CB140`/`CB141`/`CB142` de esta tesela.
    #[serde(default)]
    pub animation_frame: u8,
    /// Si la tesela está activa en el scheduler de animación.
    #[serde(default)]
    pub animation_active: bool,
    /// Bits 16..23 del random de `RoadStopScopeResolver`.
    #[serde(default)]
    pub random_bits: u8,
    /// Identidad `(GRFID, localidx)` leída de `roadstopspeclist` de un `.sav`.
    /// Se conserva hasta que el catálogo `NewGRF` activo pueda convertirla al
    /// `RoadStopSpecDef::id` local.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_grfid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_local_id: Option<u16>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Station {
    pub pos: TileCoord,
    /// `StationID` original del save de `OpenTTD`. Permite asociar en O(1) cada
    /// tesela `MP_STATION` importada con su estación, aun cuando la estación
    /// ocupe un andén grande o varias paradas unidas.
    ///
    /// Las estaciones creadas dentro del juego no tienen este identificador.
    #[serde(default)]
    pub ottd_station_id: Option<u32>,
    #[serde(default)]
    pub stop_kind: StopKind,
    /// Compañía propietaria (Fase 4; default jugador).
    #[serde(default)]
    pub owner: CompanyId,
    /// Industria asociada a una estación neutral (por ejemplo, un oil rig).
    ///
    /// `OpenTTD` mantiene el enlace inverso en `Industry::neutral_station`;
    /// `None` identifica una estación normal. El valor es el `IndustryID`
    /// nativo y no el índice del vector runtime.
    #[serde(default)]
    pub neutral_industry_id: Option<u16>,
    /// Nombre de la estación (saves de `OpenTTD` con nombre custom).
    #[serde(default)]
    pub name: Option<String>,
    /// Cargo acumulado en el almacén de la estación.
    pub stock: u32,
    #[serde(default)]
    pub cargo_stock: CargoStock,
    /// Cola de packets en espera (`StationCargoList`); fuente de verdad Fase 2.
    #[serde(default)]
    pub cargo_packets: StationCargoList,
    /// Contador histórico total de unidades entregadas (análogo a `income` simplificado).
    pub income: u64,
    /// Barridos de rating sin recogida por tipo de carga en espera.
    #[serde(default)]
    pub time_since_pickup: CargoTimeSincePickup,
    /// Estado persistente por carga (`Station::goods`): rating, velocidad y edad del último
    /// vehículo, carga en espera del barrido anterior.
    #[serde(default)]
    pub goods: super::goods_entry::StationGoods,
    /// Rating global simplificado (0–255; mayor = mejor servicio).
    #[serde(default = "default_station_rating")]
    pub rating: u8,
    /// Tipo del último vehículo que cargó aquí (`st->last_vehicle_type`); los barcos esperan
    /// cuatro veces más antes de penalizar.
    #[serde(default)]
    pub last_vehicle_type: Option<VehicleKind>,
    /// Días sin recogida por compañía (rating competitivo; default vacío).
    #[serde(default)]
    pub company_time_since_pickup: Vec<(CompanyId, CargoTimeSincePickup)>,
    /// Teselas del aeropuerto (helipuerto = `[pos]`; small = footprint completo).
    #[serde(default)]
    pub airport_tiles: Vec<TileCoord>,
    /// Gfx `AirportTile` efectivo por tesela de un aeropuerto `NewGRF`.
    ///
    /// `airport_tiles` conserva la huella y el byte `m5` conserva únicamente
    /// el `subst` vanilla para FTA/compatibilidad. Esta lista mantiene la
    /// referencia global al tile custom que debe consumir el renderer; queda
    /// vacía para aeropuertos vanilla o saves antiguos sin layout disponible.
    #[serde(default)]
    pub airport_tile_gfx: Vec<(TileCoord, u16)>,
    /// Spec de aeropuerto vanilla (`AirportSpecId`); si hay `NewGRF`, es el subst.
    #[serde(default)]
    pub airport_spec: crate::airport_class::AirportSpecId,
    /// Id global `NewGRF` del aeropuerto (`≥10`); `None` = vanilla.
    #[serde(default)]
    pub airport_newgrf_spec_id: Option<u16>,
    /// Tipo compacto `TTDPatch` de `StationScopeResolver::GetVariable(0xF1)`.
    /// Se hidrata desde Action0 `Airports` cuando el catálogo está disponible;
    /// `None` usa la agrupación vanilla derivada de `airport_spec`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub airport_ttd_type: Option<u8>,
    /// Índice de layout del aeropuerto guardado por `STNN.normal.airport.layout`.
    ///
    /// `OpenTTD` separa este selector de la orientación geométrica. Mantenerlo
    /// permite rehidratar la misma variante de un aeropuerto `NewGRF` al volver
    /// a cargar el catálogo, en vez de asumir siempre el primer layout.
    #[serde(default)]
    pub airport_layout: u8,
    /// Rotación `Direction` de `STNN.normal.airport.rotation` (`0,2,4,6`).
    #[serde(default)]
    pub airport_rotation: u8,
    /// Bloques FTA reservados (`AirportBlocks` / `st->airport.blocks`).
    #[serde(default)]
    pub airport_blocks: u64,
    /// Teselas adicionales unidas con `JoinStation` (paradas road 1×1).
    #[serde(default)]
    pub joined_tiles: Vec<TileCoord>,
    /// Spec NewGRF/vanilla usado al construir (`StationSpecId`; 0 = default).
    #[serde(default)]
    pub station_spec: crate::station_class::StationSpecId,
    /// Spec `NewGRF` de road stop al construir (`None` = vanilla / Action5 / `OpenGFX`).
    #[serde(default)]
    pub road_stop_spec: Option<u16>,
    /// Frame actual de `CBID_STATION_ANIMATION_*` para esta parada vial `NewGRF`.
    ///
    /// En este modelo cada road stop ocupa una entidad `Station`, por lo que
    /// no hace falta una tabla secundaria `roadstoptiledata` para conservarlo.
    #[serde(default)]
    pub road_stop_animation_frame: u8,
    /// La tesela está registrada en el scheduler de animación `NewGRF`.
    #[serde(default)]
    pub road_stop_animation_active: bool,
    /// Estado individual de cada tesela custom de una parada vial compuesta.
    ///
    /// Los cuatro campos legacy inmediatamente anteriores se mantienen para
    /// JSON viejo y para la ancla 1×1; esta lista es la fuente de verdad
    /// cuando existe una entrada para la tesela consultada.
    ///
    /// Se usa una lista, en vez de un mapa con `TileCoord` como clave, para
    /// que el JSON de partidas siga siendo representable por `serde_json`.
    /// Una parada vial admite como máximo 63 teselas, por lo que la búsqueda
    /// lineal no es una carga observable. Las consultas que exponen teselas
    /// ordenan explícitamente su resultado.
    #[serde(default)]
    pub road_stop_tile_states: Vec<(TileCoord, RoadStopTileState)>,
    /// Bits aleatorios de estación `NewGRF` (bits bajos de `var 5F` / Action2).
    ///
    /// `OpenTTD` conserva 16 bits para la estación; los saves históricos de
    /// openttdrs guardaban sólo un byte, que serde amplía sin pérdida.
    #[serde(default)]
    pub newgrf_random_bits: u16,
    /// Bits aleatorios legacy de la tesela ancla `RoadStop` (bits 16..23 de
    /// `RoadStopScopeResolver::GetRandomBits`). Las entradas de
    /// [`RoadStopTileState`] contienen la fuente de verdad por tesela para
    /// una parada compuesta.
    #[serde(default)]
    pub road_stop_newgrf_random_bits: u8,
    /// Máscara de eventos pendientes para grupos Action2 random (`var 5F`,
    /// byte bajo). Se limpia sólo de los triggers consumidos, permitiendo
    /// grupos `all` que esperan más de un evento.
    #[serde(default)]
    pub newgrf_waiting_random_triggers: u8,
    /// Registros persistentes `NewGRF` (`7C` / `\2psto`); writeback tras CB/Action2 (#266).
    #[serde(default)]
    pub newgrf_persistent_regs: std::collections::HashMap<u8, u32>,
    /// Índice del pool nativo `PSAC` referenciado por `STNN.normal.airport.psa`.
    ///
    /// Las estaciones creadas localmente reciben un índice al exportarse sólo
    /// cuando un callback haya escrito registros persistentes; las importadas
    /// conservan el índice original para no mezclar storages de otra entidad.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newgrf_persistent_storage_id: Option<u32>,
}

const fn default_station_rating() -> u8 {
    super::goods_entry::INITIAL_STATION_RATING
}

fn seed_station_newgrf_random_bits(pos: TileCoord) -> u16 {
    let x = pos.x.cast_unsigned();
    let y = pos.y.cast_unsigned();
    ((x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B)) >> 16) as u16
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum StopKind {
    #[default]
    TruckStop,
    BusStop,
    RailStation,
    /// Muelle (`StationType::Dock`); carga de mercancía para barcos.
    Dock,
    /// Helipuerto / aeropuerto 1×1 (`StationType::Airport`).
    Airport,
    /// Boya (`StationType::Buoy`); waypoint acuático sin carga.
    Buoy,
    /// Punto de paso ferroviario (`StationType::RailWaypoint`); sin carga ni parada.
    RailWaypoint,
    /// Punto de paso road (`StationType::RoadWaypoint`); sin carga ni parada.
    RoadWaypoint,
}

impl Station {
    #[must_use]
    pub fn new(pos: TileCoord) -> Self {
        Self::new_with_kind(pos, StopKind::TruckStop)
    }

    #[must_use]
    pub fn new_with_kind(pos: TileCoord, stop_kind: StopKind) -> Self {
        let newgrf_random_bits = seed_station_newgrf_random_bits(pos);
        Self {
            pos,
            ottd_station_id: None,
            stop_kind,
            owner: CompanyId::PLAYER,
            neutral_industry_id: None,
            name: None,
            stock: 0,
            cargo_stock: CargoStock::default(),
            cargo_packets: StationCargoList::default(),
            income: 0,
            time_since_pickup: CargoTimeSincePickup::default(),
            goods: super::goods_entry::StationGoods::default(),
            rating: default_station_rating(),
            last_vehicle_type: None,
            company_time_since_pickup: vec![(CompanyId::PLAYER, CargoTimeSincePickup::default())],
            airport_tiles: Vec::new(),
            airport_tile_gfx: Vec::new(),
            airport_spec: crate::airport_class::AirportSpecId::Heliport,
            airport_newgrf_spec_id: None,
            airport_ttd_type: None,
            airport_layout: 0,
            airport_rotation: 0,
            airport_blocks: 0,
            joined_tiles: Vec::new(),
            station_spec: crate::station_class::StationSpecId::DefaultRail,
            road_stop_spec: None,
            road_stop_animation_frame: 0,
            road_stop_animation_active: false,
            road_stop_tile_states: Vec::new(),
            newgrf_random_bits,
            road_stop_newgrf_random_bits: newgrf_random_bits.to_le_bytes()[0],
            newgrf_waiting_random_triggers: 0,
            newgrf_persistent_regs: std::collections::HashMap::new(),
            newgrf_persistent_storage_id: None,
        }
    }

    /// Bits random que expone el scope de una parada vial `NewGRF`.
    #[must_use]
    pub const fn road_stop_action2_random_bits(&self) -> u32 {
        (self.newgrf_random_bits as u32) | ((self.road_stop_newgrf_random_bits as u32) << 16)
    }

    /// Devuelve el spec `NewGRF` aplicado a una tesela vial de la estación.
    #[must_use]
    pub fn road_stop_spec_at(&self, tile: TileCoord) -> Option<u16> {
        if let Some(state) = self.road_stop_tile_state(tile) {
            return state.spec;
        }
        self.covers_tile(tile)
            .then_some(self.road_stop_spec)
            .flatten()
    }

    /// Frame de animación de una tesela vial, conservando compatibilidad con
    /// saves JSON anteriores a `road_stop_tile_states`.
    #[must_use]
    pub fn road_stop_animation_frame_at(&self, tile: TileCoord) -> u8 {
        self.road_stop_tile_state(tile)
            .map_or(self.road_stop_animation_frame, |state| {
                state.animation_frame
            })
    }

    /// Bits random propios de una tesela vial, con fallback determinista para
    /// joins que proceden de JSON anterior al mapa por tesela.
    #[must_use]
    pub fn road_stop_random_bits_at(&self, tile: TileCoord) -> u8 {
        self.road_stop_tile_state(tile).map_or_else(
            || self.legacy_road_stop_random_bits(tile),
            |state| state.random_bits,
        )
    }

    /// Estado `NewGRF` de una tesela de parada vial, si ya fue materializado.
    #[must_use]
    pub fn road_stop_tile_state(&self, tile: TileCoord) -> Option<&RoadStopTileState> {
        self.road_stop_tile_states
            .iter()
            .find_map(|(candidate, state)| (*candidate == tile).then_some(state))
    }

    /// Teselas custom de la parada, ordenadas y sin duplicados.
    #[must_use]
    pub fn road_stop_custom_tiles(&self) -> Vec<TileCoord> {
        let mut tiles: Vec<_> = self
            .road_stop_tile_states
            .iter()
            .filter_map(|(tile, state)| state.spec.map(|_| *tile))
            .collect();
        if tiles.is_empty()
            && self.road_stop_tile_states.is_empty()
            && self.road_stop_spec.is_some()
        {
            tiles.push(self.pos);
            tiles.extend(self.joined_tiles.iter().copied());
        }
        tiles.sort_unstable();
        tiles.dedup();
        tiles
    }

    /// Crea (o recupera) la entrada por tesela a partir de los campos legacy.
    /// El llamador debe invocar [`Self::sync_legacy_road_stop_anchor`] después
    /// de mutarla si también necesita mantener visibles los campos ancla.
    pub fn ensure_road_stop_tile_state(&mut self, tile: TileCoord) -> &mut RoadStopTileState {
        if let Some(index) = self
            .road_stop_tile_states
            .iter()
            .position(|(candidate, _)| *candidate == tile)
        {
            return &mut self.road_stop_tile_states[index].1;
        }
        let legacy = RoadStopTileState {
            spec: self.road_stop_spec,
            animation_frame: self.road_stop_animation_frame,
            animation_active: self.road_stop_animation_active,
            random_bits: self.legacy_road_stop_random_bits(tile),
            saved_grfid: None,
            saved_local_id: None,
        };
        let index = self.road_stop_tile_states.len();
        self.road_stop_tile_states.push((tile, legacy));
        &mut self.road_stop_tile_states[index].1
    }

    /// Expande el formato JSON anterior, que sólo retenía el estado del ancla,
    /// sobre las teselas ya unidas. Las partidas nuevas escriben entradas
    /// independientes antes de cualquier `JoinStation`.
    pub fn normalize_road_stop_tile_states(&mut self) {
        if self.road_stop_spec.is_none() && self.road_stop_tile_states.is_empty() {
            return;
        }
        let mut tiles = Vec::with_capacity(self.joined_tiles.len() + 1);
        tiles.push(self.pos);
        tiles.extend(self.joined_tiles.iter().copied());
        for tile in tiles {
            let _ = self.ensure_road_stop_tile_state(tile);
        }
        self.sync_legacy_road_stop_anchor();
    }

    /// Copia el estado de la tesela ancla a los campos mantenidos por
    /// compatibilidad. No borra un estado legacy si el ancla es vanilla.
    pub fn sync_legacy_road_stop_anchor(&mut self) {
        let Some(state) = self.road_stop_tile_state(self.pos).cloned() else {
            return;
        };
        self.road_stop_spec = state.spec;
        self.road_stop_animation_frame = state.animation_frame;
        self.road_stop_animation_active = state.animation_active;
        self.road_stop_newgrf_random_bits = state.random_bits;
    }

    fn legacy_road_stop_random_bits(&self, tile: TileCoord) -> u8 {
        if tile == self.pos {
            self.road_stop_newgrf_random_bits
        } else {
            seed_station_newgrf_random_bits(tile).to_le_bytes()[0]
        }
    }

    pub(super) fn company_pickup_slot_mut(
        &mut self,
        company: CompanyId,
    ) -> &mut CargoTimeSincePickup {
        if let Some(idx) = self
            .company_time_since_pickup
            .iter()
            .position(|(id, _)| *id == company)
        {
            return &mut self.company_time_since_pickup[idx].1;
        }
        self.company_time_since_pickup
            .push((company, CargoTimeSincePickup::default()));
        let idx = self.company_time_since_pickup.len() - 1;
        &mut self.company_time_since_pickup[idx].1
    }

    #[must_use]
    pub fn company_pickup_days(&self, company: CompanyId, cargo: CargoType) -> u8 {
        self.company_time_since_pickup
            .iter()
            .find(|(id, _)| *id == company)
            .map_or_else(|| self.time_since_pickup.get(cargo), |(_, t)| t.get(cargo))
    }

    /// Si hay balance legado sin packets, hidrata la cola (tests / saves v12).
    pub fn ensure_packets_from_stock(&mut self) {
        if self.cargo_packets.is_empty() {
            let stock = self.cargo_stock;
            if stock != CargoStock::default() {
                self.cargo_packets = StationCargoList::from_stock(stock, self.pos);
            }
        }
        self.sync_stock_from_packets();
    }

    /// Sincroniza `cargo_stock` / `stock` desde la cola de packets.
    pub fn sync_stock_from_packets(&mut self) {
        self.cargo_stock = self.cargo_packets.as_stock();
        self.stock = ALL_CARGO_TYPES
            .iter()
            .copied()
            .filter(|c| c.is_freight())
            .map(|c| self.cargo_stock.get(c))
            .fold(self.cargo_stock.custom_total(), u32::saturating_add);
    }

    /// Añade carga en espera (producción pueblo / descarga freight).
    pub fn add_waiting_cargo(&mut self, cargo: CargoType, amount: u32) {
        if amount == 0 {
            return;
        }
        let was_empty = self.cargo_stock.get(cargo) == 0;
        self.ensure_packets_from_stock();
        self.cargo_packets.add_amount(cargo, amount, self.pos);
        if was_empty {
            // Tras truncate a 255, nueva carga empieza el ciclo de antigüedad.
            self.time_since_pickup.set(cargo, 0);
        }
        self.sync_stock_from_packets();
    }

    /// Reinserta packets en espera preservando `first_station` / `feeder_paid`.
    pub fn push_waiting_packets(
        &mut self,
        packets: impl IntoIterator<Item = crate::cargo_packet::CargoPacket>,
    ) {
        self.ensure_packets_from_stock();
        for p in packets {
            if p.count == 0 {
                continue;
            }
            let cargo = p.cargo;
            let was_empty = self.cargo_stock.get(cargo) == 0;
            self.cargo_packets.push(p);
            if was_empty {
                self.time_since_pickup.set(cargo, 0);
            }
        }
        self.sync_stock_from_packets();
    }

    /// Extrae packets en espera (carga a vehículo / consumo industria).
    pub fn take_waiting_cargo(
        &mut self,
        cargo: CargoType,
        amount: u32,
    ) -> Vec<crate::cargo_packet::CargoPacket> {
        self.ensure_packets_from_stock();
        let taken = self.cargo_packets.take(cargo, amount);
        self.sync_stock_from_packets();
        taken
    }

    /// ¿La estación cubre esta tesela (ancla, aeropuerto o unidas)?
    #[must_use]
    pub fn covers_tile(&self, c: TileCoord) -> bool {
        self.pos == c || self.airport_tiles.contains(&c) || self.joined_tiles.contains(&c)
    }

    #[must_use]
    pub fn can_service_vehicle(&self, vehicle_kind: VehicleKind) -> bool {
        matches!(
            (vehicle_kind, self.stop_kind),
            (
                VehicleKind::Train,
                StopKind::RailStation | StopKind::RailWaypoint
            ) | (VehicleKind::Bus | VehicleKind::Tram, StopKind::BusStop)
                | (VehicleKind::Truck, StopKind::TruckStop)
                | (
                    VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram,
                    StopKind::RoadWaypoint,
                )
                | (VehicleKind::Ship, StopKind::Dock | StopKind::Buoy)
                | (VehicleKind::Aircraft, StopKind::Airport)
        )
    }

    #[must_use]
    pub fn is_waypoint(&self) -> bool {
        matches!(
            self.stop_kind,
            StopKind::RailWaypoint | StopKind::Buoy | StopKind::RoadWaypoint
        )
    }

    #[must_use]
    pub fn accepts_cargo(&self, cargo: CargoType) -> bool {
        if matches!(
            self.stop_kind,
            StopKind::RailWaypoint | StopKind::Buoy | StopKind::RoadWaypoint
        ) {
            return false;
        }
        match self.stop_kind {
            StopKind::BusStop => matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::TruckStop | StopKind::RailStation => {
                !matches!(cargo, CargoType::Passengers | CargoType::Mail)
            }
            // Muelle: mercancía + pasajeros (ferry).
            StopKind::Dock => true,
            StopKind::Airport => matches!(cargo, CargoType::Passengers | CargoType::Mail),
            StopKind::RailWaypoint | StopKind::Buoy | StopKind::RoadWaypoint => false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::CargoTimeSincePickup;
    use crate::CargoType;

    #[test]
    fn final_custom_time_slot_roundtrips_and_accepts_legacy_json() {
        let cargo = CargoType::Custom(32);
        let mut waiting = CargoTimeSincePickup::default();
        waiting.set(cargo, 37);
        let json = serde_json::to_string(&waiting).expect("serialize waiting age");
        let loaded: CargoTimeSincePickup =
            serde_json::from_str(&json).expect("deserialize waiting age");
        assert_eq!(loaded.get(cargo), 37);

        let mut legacy: serde_json::Value = serde_json::from_str(&json).expect("waiting age value");
        legacy["custom"] = serde_json::json!(vec![0_u8; 32]);
        let loaded_legacy: CargoTimeSincePickup =
            serde_json::from_value(legacy).expect("deserialize legacy waiting age");
        assert_eq!(loaded_legacy.get(cargo), 0);
    }
}
