//! Definición de motor / vehículo del catálogo.

use serde::{Deserialize, Serialize};

use crate::cargo::CargoType;
use crate::vehicle::VehicleKind;

/// `decay_speed` vanilla típico (20) escalado como en `engine.cpp`.
pub const DEFAULT_RELIABILITY_SPD_DEC: u16 = 80;
/// Barcos: `MS` usa `decay_speed` 5 → `20`.
pub const SHIP_RELIABILITY_SPD_DEC: u16 = 20;

/// Valor Action0 `visual_effect` que delega en la clase del motor.
pub const VEHICLE_VISUAL_EFFECT_DEFAULT: u8 = 0xFF;

fn default_reliability_spd_dec() -> u16 {
    DEFAULT_RELIABILITY_SPD_DEC
}

fn default_lifelength_years() -> u8 {
    30
}

const fn default_model_life_years() -> u8 {
    u8::MAX
}

/// Primer ID reservado para motores Action0 `NewGRF` (trains).
pub const NEWGRF_ENGINE_ID_BASE: u16 = 1000;

/// Definición de motor (paridad con `_orig_*_vehicle_info` del upstream).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
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
    /// Decaimiento diario de fiabilidad (`OpenTTD` `reliability_spd_dec`, `decay_speed << 2`).
    #[serde(default = "default_reliability_spd_dec")]
    pub reliability_spd_dec: u16,
    /// Vida útil del modelo en años de calendario (`EngineInfo::lifelength`).
    #[serde(default = "default_lifelength_years")]
    pub lifelength_years: u8,
    /// Años durante los que el modelo permanece a la venta (`EngineInfo::base_life`).
    /// `0xFF` conserva la semántica de disponibilidad ilimitada de `OpenTTD`.
    #[serde(default = "default_model_life_years")]
    pub model_life_years: u8,
    /// Unidades transferidas por tick (`EngineInfo::load_amount`). Cero usa el
    /// fallback por tipo de carga para saves y motores vanilla del port.
    #[serde(default)]
    pub load_amount: u8,
    /// Índice de sprite de locomotora (`OpenTTD` `image_index`; 0 en carretera).
    pub train_image_index: u8,
    /// `RailVehicleType::Multihead` (`engines.h`): compra spawnea cabina trasera.
    #[serde(default)]
    pub dual_headed: bool,
    /// Clase de tracción nativa (`EngineClass`): 0 vapor, 1 diésel,
    /// 2 eléctrica, 3 monorail y 4 maglev. Se usa, entre otras cosas, para
    /// seleccionar la librea ferroviaria correcta.
    #[serde(default)]
    pub rail_engine_class: u8,
    /// `EngineMiscFlag::RailIsMU`: la unidad usa el esquema DMU/EMU.
    #[serde(default)]
    pub rail_is_mu: bool,
    /// `EngineMiscFlag::Uses2CC`: el sprite admite dos colores de compañía.
    #[serde(default)]
    pub uses_2cc: bool,
    /// `EngineInfo::flags` `RailTilts` — bonus +20 % en `GetCurveSpeedLimit`.
    #[serde(default)]
    pub rail_tilts: bool,
    /// Modificador de curva en punto fijo 8.8 (`GetCurveSpeedModifier`).
    #[serde(default)]
    pub curve_speed_mod: i16,
    /// Potencia aportada por vagones motorizados (`pow_wag_power`).
    #[serde(default)]
    pub pow_wag_power: u32,
    /// Peso extra de vagones motorizados (`pow_wag_weight`).
    #[serde(default)]
    pub pow_wag_weight: u16,
    /// Procedente de Action0 Vehicles `NewGRF`.
    #[serde(default)]
    pub from_newgrf: bool,
    /// Action0 train `0x1F`: coeficiente de esfuerzo tractor (`0` = vanilla).
    #[serde(default)]
    pub tractive_effort: u8,
    /// Action0 train `0x20`: coeficiente de arrastre (`0` = fórmula por velocidad).
    #[serde(default)]
    pub air_drag: u8,
    /// Action0 train `0x21`: acorta la longitud visual (`8 - shorten_factor`).
    #[serde(default)]
    pub shorten_factor: u8,
    /// Action0 train `0x05`: índice `RailType` 0..3 (`None` = lookup vanilla por id).
    #[serde(default)]
    pub required_rail_type: Option<u8>,
    /// Action0 train `0x1D`: bitmask de cargos temperate (`0` = lista vanilla por kind).
    #[serde(default)]
    pub refit_mask: u32,
    /// Action0 `refit_cost` (factor de coste de conversión; `0` permite
    /// autorefit sin coste). Se conserva para el fallback de `CBID_VEHICLE_REFIT_COST`.
    #[serde(default)]
    pub refit_cost: u8,
    /// Cargos que el GRF incluye explícitamente mediante su CTT.
    ///
    /// A diferencia de [`Self::refit_mask`], esta lista conserva cargos
    /// custom cuyo ID global está fuera de los 32 bits históricos.
    #[serde(default)]
    pub ctt_include_cargos: Vec<CargoType>,
    /// Cargos que el GRF excluye explícitamente mediante su CTT.
    #[serde(default)]
    pub ctt_exclude_cargos: Vec<CargoType>,
    /// Action0 `cargo classes allowed` (`0x28` train, `0x1D` road, `0x18`
    /// ship/aircraft), conservado como máscara `CargoClass` de `OpenTTD`.
    #[serde(default)]
    pub cargo_classes_allowed: u16,
    /// Action0 `cargo classes disallowed`.
    #[serde(default)]
    pub cargo_classes_disallowed: u16,
    /// Action0 `cargo classes required` (`0x32`/`0x29`/`0x25`/`0x23`).
    #[serde(default)]
    pub cargo_classes_required: u16,
    /// `true` si el GRF escribió alguna propiedad de clases. Es necesario
    /// distinguir una máscara explícitamente vacía del fallback vanilla.
    #[serde(default)]
    pub cargo_classes_specified: bool,
    /// Action0 aircraft `0x09`: helicóptero.
    #[serde(default)]
    pub is_helicopter: bool,
    /// Action0 aircraft `0x0A`: avión grande.
    #[serde(default)]
    pub is_large_aircraft: bool,
    /// Action0 miscellaneous flag bit 7: render a `NewGRF` sprite sequence.
    #[serde(default)]
    pub sprite_stack: bool,
    /// Action0 ship `0x14`: fracción de velocidad en océano (`0` = 256/256).
    #[serde(default)]
    pub ocean_speed_frac: u8,
    /// Action0 ship `0x15`: fracción de velocidad en canal (`0` = 256/256).
    #[serde(default)]
    pub canal_speed_frac: u8,
    /// Action0 RV `0x12` / ship `0x10` / aircraft `0x12` (`0`/`0xFF` = default).
    #[serde(default)]
    pub sound_effect: u8,
    /// Action0 visual effect (`train 0x22`, `road 0x21`, `ship 0x1C`).
    /// `0xFF` conserva la selección por clase de motor de `OpenTTD`.
    #[serde(default = "default_visual_effect")]
    pub visual_effect: u8,
    /// Vistas Action1 (1..=8); vacías = sin gfx `NewGRF`. No se serializa en saves.
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Id local Action3 en el GRF (para re-resolver Action2 en runtime).
    ///
    /// `OpenTTD` lee este campo como *extended byte*: los IDs `0x00..=0xFE`
    /// ocupan un byte y `0xFF` introduce un WORD.  Mantenerlo en `u16` evita
    /// truncar los motores que sólo aparecen como destino de CB16.
    #[serde(default, skip)]
    pub newgrf_local_id: u16,
    /// Graphics completas si hace falta re-resolver random/advanced al dibujar.
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
    /// GRFID del `NewGRF` que definió este motor (0 = vanilla).
    #[serde(default, skip)]
    pub newgrf_grfid: u32,
    /// Versión del formato `NewGRF` que declaró el motor (Action8).
    ///
    /// Se conserva sólo en runtime: `CBID_VEHICLE_CUSTOM_REFIT` necesita
    /// distinguir el fallback de CTT de GRF v1..6 del `bitnum` de v7+.
    #[serde(default, skip)]
    pub newgrf_grf_version: u8,
    /// Tablas de traducción del GRF que definió el motor. La CTT es parte del
    /// contexto de la evaluación de callbacks y no se serializa en saves.
    #[serde(default, skip)]
    pub newgrf_type_tables: Option<crate::newgrf_type_tables::GrfTypeTranslationTables>,
    /// Máscara Action0 de callbacks de vehículo; bit 7 = `SoundEffect`.
    #[serde(default)]
    pub vehicle_callback_mask: u16,
    /// Badges globales asociados al motor (`ReadBadgeList`).
    #[serde(default)]
    pub badges: Vec<u16>,
    /// Traducción de índice local del GRF a id global de badge. No se persiste:
    /// se reconstruye al aplicar el stack `NewGRF`.
    #[serde(default, skip)]
    pub newgrf_badge_translation: Vec<u16>,
}

const fn default_visual_effect() -> u8 {
    VEHICLE_VISUAL_EFFECT_DEFAULT
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
        let views = runtime.views_for_local_id_u16_ctx(self.newgrf_local_id, ctx)?;
        if views.is_empty() {
            return None;
        }
        Some(views[dir % views.len()].clone())
    }

    /// Vista `NewGRF` aplicando el *wagon override* del motor que encabeza el
    /// consist y cayendo al grupo propio cuando no hay una coincidencia.
    pub fn newgrf_view_runtime_with_override(
        &self,
        dir: usize,
        cargo: Option<crate::cargo::CargoType>,
        overriding_local_id: Option<u16>,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::DecodedSprite> {
        let runtime = self.newgrf_runtime.as_ref()?;
        let views = overriding_local_id
            .and_then(|overriding_id| {
                runtime.views_for_wagon_override_u16_ctx(
                    self.newgrf_local_id,
                    overriding_id,
                    cargo,
                    ctx,
                )
            })
            .or_else(|| {
                runtime.views_for_local_id_cargo_u16_ctx(self.newgrf_local_id, cargo, ctx)
            })?;
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
