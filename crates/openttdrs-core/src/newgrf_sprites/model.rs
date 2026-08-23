//! Tipos de datos para sprites `NewGRF` y grafos Action2.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::action2::{eval_action2_random, eval_action2_var, resolve_callback_chain};

/// Sprite RGBA decodificado (índice 0 → alpha 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedSprite {
    pub width: u16,
    pub height: u16,
    pub x_offs: i16,
    pub y_offs: i16,
    /// `width * height * 4` bytes RGBA.
    pub rgba: Vec<u8>,
    /// Máscara 8bpp (mismo `width*height`); vacío si no hay.
    #[serde(default)]
    pub mask: Vec<u8>,
}

/// Asignación Action3: id local → set Action2 (o índice Action1 si no hay Action2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainSpriteAssign {
    pub local_id: u8,
    pub set_id: u16,
}

/// Ajuste `varadjust` (shift/and [+add+div|mod]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Action2VarAdjust {
    /// Bits 0..4 del `shift-num`.
    pub shift: u8,
    pub and_mask: u8,
    pub add_val: Option<u8>,
    pub divide_val: Option<u8>,
    pub modulo_val: Option<u8>,
}

/// Un término variable + ajuste (y parámetro opcional para `60+x`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2VarTerm {
    pub variable: u8,
    /// Parámetro tras variables `60+x` (p. ej. registro `7D`).
    pub param: Option<u8>,
    pub adjust: Action2VarAdjust,
}

/// Operación advanced: `operator` entre acumulador y el siguiente término.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2VarOp {
    pub operator: u8,
    pub rhs: Action2VarTerm,
}

/// Action2 variational (`0x81`/`0x82`): variable + rangos + default.
///
/// Con bit 5 en `shift-num` se encadena `ops` (advanced). Sin bit 5, `ops` vacío.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2VarEntry {
    pub first: Action2VarTerm,
    /// Cadena advanced (`operator` + término); vacía = variational simple.
    pub ops: Vec<Action2VarOp>,
    /// `(result_set, low, high)` inclusive.
    pub ranges: Vec<(u16, u8, u8)>,
    pub default: u16,
}

/// Action2 random (`0x80`/`0x83`/`0x84`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2RandomEntry {
    /// `0x80` propio, `0x83` related, `0x84` consist.
    pub typ: u8,
    /// Solo `0x84`: conteo desde vehículo de control (nibble bajo = offset).
    pub consist_count: u8,
    pub triggers: u8,
    pub randbit: u8,
    pub sets: Vec<u16>,
}

/// Contexto para evaluar variational / random (preview o runtime).
#[derive(Debug, Clone, Default)]
pub struct Action2EvalCtx {
    /// Valores de variables `NewGRF` (`variable` → raw).
    pub vars: HashMap<u8, u32>,
    /// Bits aleatorios del objeto (vehículo/estación/…).
    pub random_bits: u32,
    /// Bits de vehículos del consist indexados por offset (`0x84` nibble bajo).
    pub consist_random_bits: HashMap<u8, u32>,
    /// Registros temporales (variable `7D` / operador `\2sto`).
    pub temp_registers: HashMap<u8, u32>,
    /// Registros persistentes (variable `7C` / operador `\2psto`).
    pub persistent_registers: HashMap<u8, u32>,
    /// Último resultado de un `VarAction2` (variable `1C`; p. ej. tras procedure `7E`).
    pub last_result: u32,
    /// Parámetros del GRF (`GRFFile::param`; variable `0x7F[param]`).
    pub grf_params: Vec<u32>,
}

impl Action2EvalCtx {
    /// Copia parámetros del stack para resolución Action2 (`0x7F`).
    pub fn set_grf_params(&mut self, params: &[u32]) {
        self.grf_params.clear();
        self.grf_params.extend_from_slice(params);
    }
}

/// Resultado de parsear Action1/2/3 de un feature (trains / roadtypes).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrainSpriteGraphics {
    /// `sets[set_id][view]` — sets Action1 en orden de aparición.
    pub sets: Vec<Vec<DecodedSprite>>,
    pub assigns: Vec<TrainSpriteAssign>,
    /// Action3 específico: `(id local, cargo/sprite-type)` → set Action2.
    ///
    /// Para `RailTypes` el segundo byte es `RailSpriteType`; señales usan `11`.
    pub specific_assigns: HashMap<(u8, u8), u16>,
    /// Action2 set-id → índice del primer set Action1 "moving" (solo trains).
    pub action2_to_action1: HashMap<u8, u16>,
    /// Action2 variational completo (rangos + default / advanced).
    pub action2_var: HashMap<u8, Action2VarEntry>,
    /// Action2 random (`0x80`/`0x83`/`0x84`).
    pub action2_random: HashMap<u8, Action2RandomEntry>,
}

impl TrainSpriteGraphics {
    /// Preview (primera vista) para un id local.
    #[must_use]
    pub fn preview_for_local_id(&self, local_id: u8) -> Option<&DecodedSprite> {
        self.views_for_local_id(local_id)?.first()
    }

    /// Resuelve sin contexto (variational → `default`; random → set\[0]).
    #[must_use]
    pub fn resolve_action1_set(&self, action3_set_id: u16) -> u16 {
        self.resolve_action1_set_ctx(action3_set_id, &mut Action2EvalCtx::default())
    }

    /// Resuelve Action3 → var/random → Action2 básico → Action1.
    pub fn resolve_action1_set_ctx(&self, action3_set_id: u16, ctx: &mut Action2EvalCtx) -> u16 {
        let mut id = action3_set_id;
        for _ in 0..8 {
            let a2 = u8::try_from(id).unwrap_or(u8::MAX);
            if let Some(rnd) = self.action2_random.get(&a2) {
                let next = eval_action2_random(rnd, ctx);
                if next & 0x8000 != 0 {
                    break;
                }
                id = next;
                continue;
            }
            if let Some(var) = self.action2_var.get(&a2).cloned() {
                let next = eval_action2_var(self, &var, ctx, 0);
                if next & 0x8000 != 0 {
                    break;
                }
                id = next;
                continue;
            }
            if let Some(&a1) = self.action2_to_action1.get(&a2) {
                return a1;
            }
            return id;
        }
        self.action2_to_action1
            .get(&u8::try_from(id).unwrap_or(u8::MAX))
            .copied()
            .unwrap_or(id)
    }

    /// Todas las vistas del set asignado al id local (ctx por defecto).
    #[must_use]
    pub fn views_for_local_id(&self, local_id: u8) -> Option<&[DecodedSprite]> {
        self.views_for_local_id_ctx(local_id, &mut Action2EvalCtx::default())
    }

    /// Vistas resolviendo Action2 con contexto (random/consist/advanced).
    pub fn views_for_local_id_ctx(
        &self,
        local_id: u8,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        let set_id = self
            .assigns
            .iter()
            .find(|a| a.local_id == local_id)
            .map(|a| a.set_id)
            .or_else(|| (!self.sets.is_empty()).then_some(0))?;
        let action1_idx = self.resolve_action1_set_ctx(set_id, ctx);
        self.sets
            .get(usize::from(action1_idx))
            .map(Vec::as_slice)
            .filter(|s| !s.is_empty())
    }

    /// Vistas del grupo Action3 específico (p. ej. `RailType` Signals = selector 11).
    pub fn views_for_specific_ctx(
        &self,
        local_id: u8,
        selector: u8,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        let set_id = *self.specific_assigns.get(&(local_id, selector))?;
        let action1_idx = self.resolve_action1_set_ctx(set_id, ctx);
        self.sets
            .get(usize::from(action1_idx))
            .map(Vec::as_slice)
            .filter(|s| !s.is_empty())
    }

    /// Resuelve el grupo Action3 específico de una carga y usa el grupo default
    /// cuando esa carga no tiene asignación. Es la selección de sprites de los
    /// features de vehículos de `OpenTTD`.
    pub fn views_for_local_id_cargo_ctx(
        &self,
        local_id: u8,
        cargo: Option<crate::cargo::CargoType>,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        if let Some(selector) = cargo.map(crate::cargo::CargoType::temperate_id)
            && self.has_specific_assignment(local_id, selector)
        {
            return self.views_for_specific_ctx(local_id, selector, ctx);
        }
        self.views_for_local_id_ctx(local_id, ctx)
    }

    /// ¿Action3 asignó este grupo específico al id local?
    #[must_use]
    pub fn has_specific_assignment(&self, local_id: u8, selector: u8) -> bool {
        self.specific_assigns.contains_key(&(local_id, selector))
    }

    /// ¿Necesita re-resolución en runtime (random o cualquier variational)?
    #[must_use]
    pub fn needs_runtime_resolve(&self) -> bool {
        !self.action2_random.is_empty() || !self.action2_var.is_empty()
    }

    /// Resuelve un callback `NewGRF` (`nvar=0` → valor; sprite group → [`CALLBACK_FAILED`]).
    ///
    /// Inserta `0x0C`/`0x10`/`0x18` en el contexto (como `ResolverObject` upstream).
    #[must_use]
    pub fn resolve_callback(&self, local_id: u8, callback: u16, param1: u32, param2: u32) -> u16 {
        let mut ctx = Action2EvalCtx::default();
        self.resolve_callback_ctx(local_id, callback, param1, param2, &mut ctx)
    }

    /// Como [`Self::resolve_callback`], pero reutiliza/muta `ctx` (regs persistentes, etc.).
    ///
    /// Fallo sin cadena callback → [`CALLBACK_FAILED`] (observable; no silencioso).
    pub fn resolve_callback_ctx(
        &self,
        local_id: u8,
        callback: u16,
        param1: u32,
        param2: u32,
        ctx: &mut Action2EvalCtx,
    ) -> u16 {
        let set_id = self
            .assigns
            .iter()
            .find(|a| a.local_id == local_id)
            .map(|a| a.set_id)
            .or_else(|| (!self.sets.is_empty()).then_some(0));
        let Some(set_id) = set_id else {
            return CALLBACK_FAILED;
        };
        ctx.vars.insert(0x0C, u32::from(callback));
        ctx.vars.insert(0x10, param1);
        ctx.vars.insert(0x18, param2);
        resolve_callback_chain(self, set_id, ctx)
    }
}

/// Resultado "callback fallido" (`OpenTTD` `CALLBACK_FAILED`).
pub const CALLBACK_FAILED: u16 = 0xFFFF;
/// Callback estaciones: layout de tesela al construir (`CBID_STATION_BUILD_TILE_LAYOUT`).
pub const CBID_STATION_BUILD_TILE_LAYOUT: u16 = 0x24;
/// Callback vehículos: permitir start/stop (`CBID_VEHICLE_START_STOP_CHECK`).
pub const CBID_VEHICLE_START_STOP_CHECK: u16 = 0x31;
/// Callback industrias: disponibilidad / ubicación al colocar (`CBID_INDUSTRY_LOCATION`).
pub const CBID_INDUSTRY_LOCATION: u16 = 0x28;
/// Callback casas: permitir construcción (`CBID_HOUSE_ALLOW_CONSTRUCTION`).
pub const CBID_HOUSE_ALLOW_CONSTRUCTION: u16 = 0x17;
/// Callback cargos: calcular ingreso de la entrega (`CBID_CARGO_PROFIT_CALC`).
pub const CBID_CARGO_PROFIT_CALC: u16 = 0x39;
/// Callback cargos: calcular rating de estación (`CBID_CARGO_STATION_RATING_CALC`).
pub const CBID_CARGO_STATION_RATING_CALC: u16 = 0x145;
/// Callback objetos: comprobar pendiente de cada tesela (`CBID_OBJECT_LAND_SLOPE_CHECK`).
pub const CBID_OBJECT_LAND_SLOPE_CHECK: u16 = 0x157;
/// Callback teselas industria: trigger de animación (`CBID_INDTILE_ANIMATION_TRIGGER`).
pub const CBID_INDTILE_ANIMATION_TRIGGER: u16 = 0x25;
/// Callback teselas industria: siguiente frame (`CBID_INDTILE_ANIMATION_NEXT_FRAME`).
pub const CBID_INDTILE_ANIMATION_NEXT_FRAME: u16 = 0x26;
/// Callback teselas industria: velocidad de animación (`CBID_INDTILE_ANIMATION_SPEED`).
pub const CBID_INDTILE_ANIMATION_SPEED: u16 = 0x27;
/// Alias de compatibilidad para el nombre histórico del callback de siguiente frame.
pub const CBID_INDTILE_ANIM_NEXT_FRAME: u16 = CBID_INDTILE_ANIMATION_NEXT_FRAME;
/// Callback estaciones: disponibilidad de clase/spec (`CBID_STATION_AVAILABILITY`).
pub const CBID_STATION_AVAILABILITY: u16 = 0x13;
/// Callback estaciones: comprobar pendiente de cada tesela al construir.
pub const CBID_STATION_LAND_SLOPE_CHECK: u16 = 0x149;
/// Callback estaciones / road stops: inicia, pausa o fija un frame de animación.
pub const CBID_STATION_ANIMATION_TRIGGER: u16 = 0x140;
/// Callback estaciones / road stops: siguiente frame de animación.
pub const CBID_STATION_ANIMATION_NEXT_FRAME: u16 = 0x141;
/// Callback estaciones / road stops: velocidad de animación.
pub const CBID_STATION_ANIMATION_SPEED: u16 = 0x142;

/// Bloque de sprites Action5 (shore / catenary / …).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Action5Block {
    /// Tipo (`0x0D` = shore, `0x05` = catenary, …).
    pub type_id: u8,
    pub offset: u16,
    pub num_sprites: u8,
    /// Sprites decodificados (puede estar vacío si no hay data section).
    pub sprites: Vec<DecodedSprite>,
    /// Preview del primer sprite (si está disponible).
    pub first_preview: Option<DecodedSprite>,
}
