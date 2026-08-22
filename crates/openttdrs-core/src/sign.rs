//! Carteles del mapa (`Sign` / `CmdPlaceSign` en `OpenTTD`).
//!
//! Entidades ligeras con posición y texto; no mutan la tesela.

use serde::{Deserialize, Serialize};

use crate::company::CompanyId;
use crate::map::TileCoord;

/// Longitud máxima del texto del cartel (caracteres Unicode).
pub const MAX_SIGN_NAME_CHARS: usize = 32;

/// Propietario lógico de un cartel de mapa.
///
/// `OpenTTD` reserva dos owners que no son compañías: `OWNER_NONE` para
/// carteles sin dueño (por ejemplo, de una compañía eliminada) y `OWNER_DEITY`
/// para carteles creados por `GameScript`. Modelarlos evita que el filtro de
/// competidores trate ambos como si fueran la compañía local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignOwner {
    Company(CompanyId),
    Unowned,
    Deity,
}

impl Default for SignOwner {
    fn default() -> Self {
        Self::Company(CompanyId::PLAYER)
    }
}

impl SignOwner {
    /// Reproduce el filtro `DO_SHOW_COMPETITOR_SIGNS` del viewport de `OpenTTD`.
    ///
    /// Los carteles de `GameScript` siguen visibles aun con competidores ocultos;
    /// los carteles sin dueño se ocultan, igual que los restos de una compañía
    /// quebrada en el cliente oficial.
    #[must_use]
    pub fn visible_to(self, local_company: CompanyId, show_competitors: bool) -> bool {
        show_competitors
            || matches!(self, Self::Company(owner) if owner == local_company)
            || matches!(self, Self::Deity)
    }
}

/// Cartel colocado por el jugador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sign {
    pub id: u32,
    pub pos: TileCoord,
    pub name: String,
    /// Dueño para color y filtro de competidores. Los saves JSON anteriores
    /// cargan como cartel de la compañía jugador mediante [`SignOwner::default`].
    #[serde(default)]
    pub owner: SignOwner,
}

impl Sign {
    #[must_use]
    pub fn new(id: u32, pos: TileCoord, name: impl Into<String>) -> Self {
        Self::new_owned(id, pos, name, CompanyId::PLAYER)
    }

    #[must_use]
    pub fn new_owned(id: u32, pos: TileCoord, name: impl Into<String>, owner: CompanyId) -> Self {
        Self {
            id,
            pos,
            name: name.into(),
            owner: SignOwner::Company(owner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn competitor_visibility_matches_openttd_owner_rules() {
        let local = CompanyId::PLAYER;
        let rival = CompanyId(1);
        assert!(SignOwner::Company(local).visible_to(local, false));
        assert!(!SignOwner::Company(rival).visible_to(local, false));
        assert!(!SignOwner::Unowned.visible_to(local, false));
        assert!(SignOwner::Deity.visible_to(local, false));
        assert!(SignOwner::Company(rival).visible_to(local, true));
    }

    #[test]
    fn new_owned_sign_keeps_its_company() {
        let sign = Sign::new_owned(3, TileCoord::new(2, 4), "Mirador", CompanyId(2));
        assert_eq!(sign.owner, SignOwner::Company(CompanyId(2)));
    }
}
