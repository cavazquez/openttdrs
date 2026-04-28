use crate::map::TileCoord;

#[derive(Debug, Clone)]
pub struct Station {
    pub pos: TileCoord,
    /// Cargo acumulado en el almacén de la estación.
    pub stock: u32,
    /// Contador histórico total de unidades entregadas (análogo a `income` simplificado).
    pub income: u64,
}

impl Station {
    #[must_use]
    pub fn new(pos: TileCoord) -> Self {
        Self {
            pos,
            stock: 0,
            income: 0,
        }
    }
}
