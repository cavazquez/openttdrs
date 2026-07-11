//! Carteles del mapa (`Sign` / `CmdPlaceSign` en `OpenTTD`).
//!
//! Entidades ligeras con posición y texto; no mutan la tesela.

use serde::{Deserialize, Serialize};

use crate::map::TileCoord;

/// Longitud máxima del texto del cartel (caracteres Unicode).
pub const MAX_SIGN_NAME_CHARS: usize = 32;

/// Cartel colocado por el jugador.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sign {
    pub id: u32,
    pub pos: TileCoord,
    pub name: String,
}

impl Sign {
    #[must_use]
    pub fn new(id: u32, pos: TileCoord, name: impl Into<String>) -> Self {
        Self {
            id,
            pos,
            name: name.into(),
        }
    }
}
