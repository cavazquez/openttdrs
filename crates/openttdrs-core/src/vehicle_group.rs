//! Grupos de vehículos (paridad reducida con `OpenTTD` `group_cmd`).

pub const MAX_VEHICLE_GROUP_NAME_CHARS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VehicleGroup {
    /// Identificador de pool (`GroupID`) usado por `Vehicle::group_id`.
    pub id: u32,
    pub name: String,
    /// Empresa propietaria (`INVALID_OWNER` se representa como `0xFF`).
    #[serde(default)]
    pub owner: u8,
    /// Tipo de vehículo (`VehicleType` de `OpenTTD`).
    #[serde(default)]
    pub vehicle_type: u8,
    /// Flags persistentes de autoreemplazo del grupo.
    #[serde(default)]
    pub flags: u8,
    /// Estado de librea persistido por `OpenTTD`.
    #[serde(default)]
    pub livery_in_use: u8,
    #[serde(default)]
    pub livery_colour1: u8,
    #[serde(default)]
    pub livery_colour2: u8,
    /// Grupo padre, si pertenece a una jerarquía.
    #[serde(default)]
    pub parent: Option<u32>,
    /// Número por empresa (distinto del `GroupID` de pool).
    #[serde(default)]
    pub number: u32,
}

impl VehicleGroup {
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            owner: 0,
            vehicle_type: 0,
            flags: 0,
            livery_in_use: 0,
            livery_colour1: 0,
            livery_colour2: 0,
            parent: None,
            number: id,
        }
    }
}

#[must_use]
pub fn next_vehicle_group_id(groups: &[VehicleGroup]) -> u32 {
    groups
        .iter()
        .map(|g| g.id)
        .max()
        .map_or(1, |id| id.saturating_add(1))
}
