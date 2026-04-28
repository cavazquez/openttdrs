use crate::map::TileCoord;

/// Ticks entre cada ciclo de producción (equivale a `INDUSTRY_PRODUCE_TICKS` del upstream).
pub const INDUSTRY_PRODUCE_TICKS: u64 = 256;

/// Unidades producidas por ciclo.
pub const INDUSTRY_PRODUCE_AMOUNT: u32 = 8;

/// Capacidad máxima de stock por defecto.
pub const INDUSTRY_STOCK_CAPACITY: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndustryKind {
    CoalMine,
    Forest,
}

#[derive(Debug, Clone)]
pub struct Industry {
    pub pos:      TileCoord,
    pub kind:     IndustryKind,
    pub stock:    u32,
    pub capacity: u32,
}

impl Industry {
    #[must_use]
    pub fn new(pos: TileCoord, kind: IndustryKind) -> Self {
        Self {
            pos,
            kind,
            stock: 0,
            capacity: INDUSTRY_STOCK_CAPACITY,
        }
    }

    /// Produce cargo si el tick actual es múltiplo del período de producción.
    pub fn produce(&mut self, tick: u64) {
        if tick > 0 && tick % INDUSTRY_PRODUCE_TICKS == 0 {
            self.stock = self.stock.saturating_add(INDUSTRY_PRODUCE_AMOUNT).min(self.capacity);
        }
    }
}
