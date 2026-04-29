use crate::map::TileCoord;

/// Ticks entre cada ciclo de producción (equivale a `INDUSTRY_PRODUCE_TICKS` del upstream).
pub const INDUSTRY_PRODUCE_TICKS: u64 = 256;

/// Unidades producidas por ciclo.
pub const INDUSTRY_PRODUCE_AMOUNT: u32 = 8;

/// Capacidad máxima de stock por defecto.
pub const INDUSTRY_STOCK_CAPACITY: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IndustryKind {
    CoalMine,
    Forest,
    /// Extracción liviana (pozos de petróleo, etc.): mismo ritmo de stock que mina.
    OilWell,
    /// Procesamiento: produce la mitad de frecuencia que mina/bosque.
    Factory,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Industry {
    pub pos: TileCoord,
    pub kind: IndustryKind,
    pub stock: u32,
    pub capacity: u32,
}

#[inline]
#[must_use]
pub const fn industry_produce_period_ticks(kind: IndustryKind) -> u64 {
    match kind {
        IndustryKind::Factory => INDUSTRY_PRODUCE_TICKS * 2,
        IndustryKind::CoalMine | IndustryKind::Forest | IndustryKind::OilWell => {
            INDUSTRY_PRODUCE_TICKS
        }
    }
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

    /// Produce cargo si el tick actual es múltiplo del período de producción (las fábricas van más lento).
    pub fn produce(&mut self, tick: u64) {
        let period = industry_produce_period_ticks(self.kind);
        if tick > 0 && tick.is_multiple_of(period) {
            self.stock = self
                .stock
                .saturating_add(INDUSTRY_PRODUCE_AMOUNT)
                .min(self.capacity);
        }
    }
}
