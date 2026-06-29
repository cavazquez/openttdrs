//! Grupos de vehículos (paridad reducida con `OpenTTD` `group_cmd`).

pub const MAX_VEHICLE_GROUP_NAME_CHARS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VehicleGroup {
    pub id: u32,
    pub name: String,
}

impl VehicleGroup {
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
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
