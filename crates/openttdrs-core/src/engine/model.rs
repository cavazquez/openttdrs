//! Definición de motor / vehículo del catálogo.

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::vehicle::VehicleKind;

/// Primer ID reservado para motores Action0 `NewGRF` (trains).
pub const NEWGRF_ENGINE_ID_BASE: u16 = 1000;

/// Definición de motor (paridad con `_orig_*_vehicle_info` del upstream).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineDef {
    pub id: u16,
    pub kind: VehicleKind,
    pub name: String,
    /// Unidades `OpenTTD` (`RVI` ≈ 1 km/h por unidad; `ROV` ≈ 0,5 km/h).
    pub max_speed: u16,
    /// Precio de compra (libras internas TTD: `base_price × cost_factor >> 8`).
    pub price: i64,
    /// Coste de explotación anual (libras internas TTD).
    pub running_cost_year: i64,
    /// Capacidad del modelo (pasajeros/sacas/cajas). 0 = solo locomotora.
    pub capacity: u32,
    /// Carga de diseño del modelo (`None` = locomotora sin carga propia).
    pub cargo: Option<CargoType>,
    pub power_hp: u32,
    pub weight_t: u16,
    pub intro_year: u16,
    /// Fiabilidad inicial mostrada en la compra (aprox. por clase de motor).
    pub reliability_pct: u8,
    /// Índice de sprite de locomotora (`OpenTTD` `image_index`; 0 en carretera).
    pub train_image_index: u8,
    /// `RailVehicleType::Multihead` (`engines.h`): compra spawnea cabina trasera.
    #[serde(default)]
    pub dual_headed: bool,
    /// Procedente de Action0 Vehicles `NewGRF`.
    #[serde(default)]
    pub from_newgrf: bool,
    /// Vistas Action1 (1..=8); vacías = sin gfx `NewGRF`. No se serializa en saves.
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Id local Action3 en el GRF (para re-resolver Action2 en runtime).
    #[serde(default, skip)]
    pub newgrf_local_id: u8,
    /// Graphics completas si hace falta re-resolver random/advanced al dibujar.
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
    /// GRFID del `NewGRF` que definió este motor (0 = vanilla).
    #[serde(default, skip)]
    pub newgrf_grfid: u32,
}

impl EngineDef {
    /// Preview de compra: primera vista `NewGRF`, si hay.
    #[must_use]
    pub fn newgrf_preview(&self) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        self.newgrf_views.first()
    }

    /// Vista para dirección de render (`dir` 0..=7); con 1 sola vista se reutiliza.
    #[must_use]
    pub fn newgrf_view(&self, dir: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return None;
        }
        let i = dir % self.newgrf_views.len();
        self.newgrf_views.get(i)
    }

    /// Vista `NewGRF` re-resolviendo Action2 random/advanced con contexto de consist.
    pub fn newgrf_view_runtime(
        &self,
        dir: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::DecodedSprite> {
        let runtime = self.newgrf_runtime.as_ref()?;
        let views = runtime.views_for_local_id_ctx(self.newgrf_local_id, ctx)?;
        if views.is_empty() {
            return None;
        }
        Some(views[dir % views.len()].clone())
    }

    /// Velocidad máxima en km/h para mostrar en UI (conversión por tipo).
    #[must_use]
    pub fn speed_kmh(&self) -> u16 {
        match self.kind {
            VehicleKind::Train | VehicleKind::Aircraft => self.max_speed,
            VehicleKind::Bus | VehicleKind::Truck | VehicleKind::Tram | VehicleKind::Ship => {
                self.max_speed / 2
            }
        }
    }

    /// Vagón: tren sin potencia y con capacidad de carga.
    #[must_use]
    pub fn is_wagon(&self) -> bool {
        matches!(self.kind, VehicleKind::Train) && self.power_hp == 0 && self.capacity > 0
    }

    /// Locomotora o DMU (puede ser cabeza de consist).
    #[must_use]
    pub fn is_train_engine(&self) -> bool {
        matches!(self.kind, VehicleKind::Train) && !self.is_wagon()
    }

    /// Multihead vanilla (`RailVehicleType::Multihead`).
    #[must_use]
    pub fn is_dual_headed(&self) -> bool {
        self.dual_headed
    }
}
