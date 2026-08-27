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

/// Referencia a un sprite dentro de un layout `TileSeq` de Action2.
///
/// En el formato `NewGRF`, el bit 15 de la paleta indica que el campo `sprite`
/// no es un id absoluto sino el índice de un set Action1. El parser conserva
/// ambos casos: los ids vanilla/directos se mantienen para diagnóstico y los
/// sets Action1 se resuelven contra [`TrainSpriteGraphics::sets`] al dibujar.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileLayoutSpriteRef {
    /// Índice del set Action1 cuando el sprite usa el marcador custom.
    pub action1_set: Option<u16>,
    /// Id absoluto para sprites del banco base (no custom).
    pub direct_sprite: u16,
    /// Índice del set Action1 usado como paleta explícita, si existe.
    pub palette_action1_set: Option<u16>,
    /// Paleta absoluta del layout cuando no referencia Action1.
    pub direct_palette: u16,
    /// Flags `TileLayoutFlags` del registro de layout.
    pub flags: u8,
    /// Origen `TILE_SEQ` en unidades de mapa/píxel. `origin_z == -128`
    /// identifica un child (`DrawTileSeqStruct::IsParentSprite == false`).
    pub origin: [i8; 3],
    /// Extensión de la caja cuando la entrada es parent.
    pub extent: [u8; 3],
}

impl TileLayoutSpriteRef {
    /// `true` si la entrada crea un parent sortable con caja 3D.
    #[must_use]
    pub const fn is_parent(&self) -> bool {
        self.origin[2] != i8::MIN
    }
}

/// Layout `NewGRF` completo: suelo y secuencia de building/child sprites.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileLayout {
    pub ground: TileLayoutSpriteRef,
    pub sequence: Vec<TileLayoutSpriteRef>,
}

/// Sprite de un layout después de resolver su referencia Action1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTileLayoutSprite {
    pub sprite: DecodedSprite,
    pub origin: [i8; 3],
    pub extent: [u8; 3],
}

impl ResolvedTileLayoutSprite {
    #[must_use]
    pub const fn is_parent(&self) -> bool {
        self.origin[2] != i8::MIN
    }
}

/// Layout listo para que el cliente cree el suelo y la secuencia de parents /
/// children. Las entradas directas que no pertenecen a un sprite Action1 no
/// tienen una textura decodificada y se omiten del resultado.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedTileLayout {
    pub ground: Option<ResolvedTileLayoutSprite>,
    pub sequence: Vec<ResolvedTileLayoutSprite>,
    /// `false` when an entry needs a base sprite, custom palette or register
    /// preprocessing that the compact client model does not expose. Consumers
    /// must use the vanilla path for incomplete layouts.
    pub complete: bool,
}

impl TileLayout {
    fn resolve(&self, graphics: &TrainSpriteGraphics, view: usize) -> ResolvedTileLayout {
        let mut complete = true;
        let mut resolve_sprite = |reference: &TileLayoutSpriteRef| {
            // Direct base-set sprites and register-driven layouts need the
            // original sprite/palette resolver. The decoded Action1 cache
            // cannot reproduce those yet; mark the whole layout incomplete so
            // callers can fall back atomically instead of drawing a mixture
            // of custom and vanilla pieces.
            if reference.flags != 0 {
                complete = false;
                return None;
            }
            if reference.action1_set.is_none() {
                if reference.direct_sprite != 0 {
                    complete = false;
                }
                return None;
            }
            let set = reference.action1_set?;
            let Some(sprites) = graphics.sets.get(usize::from(set)) else {
                complete = false;
                return None;
            };
            // Road-stop layouts select orientation in the Action2 resolver;
            // the selected custom reference is the first sprite of its
            // Action1 set. Construction-stage selection for houses/objects is
            // intentionally left to their future feature-specific processor.
            let _ = view;
            let Some(sprite) = sprites.first().cloned() else {
                complete = false;
                return None;
            };
            Some(ResolvedTileLayoutSprite {
                sprite,
                origin: reference.origin,
                extent: reference.extent,
            })
        };

        ResolvedTileLayout {
            ground: resolve_sprite(&self.ground),
            sequence: self.sequence.iter().filter_map(resolve_sprite).collect(),
            complete,
        }
    }
}

/// Asignación Action3: id local → set Action2 (o índice Action1 si no hay Action2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrainSpriteAssign {
    pub local_id: u8,
    pub set_id: u16,
}

/// Asignación de un grupo Action3 de *wagon override*.
///
/// `wagon_local_id` identifica el vehículo cuyo sprite se reemplaza y
/// `overriding_local_id` uno de los motores de la cadena Action3 anterior.
/// `selector` es el cargo (o `0xFF` para el grupo default). Los IDs usan el
/// mismo formato extendido que Action3, por eso ambos son `u16`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WagonOverrideAssign {
    pub wagon_local_id: u16,
    pub overriding_local_id: u16,
    pub selector: u8,
    pub set_id: u16,
}

/// Action2 real group for vehicle graphics.
///
/// `OpenTTD` keeps separate sprite-set choices while a vehicle is moving and
/// while it is in a loading window.  The old compact graph only retained the
/// first word of this group, which made every cargo/loading stage look like
/// the empty vehicle.  The entries are Action1 set ids (not sprite ids).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Action2RealEntry {
    pub loaded: Vec<u16>,
    pub loading: Vec<u16>,
}

/// Grupo Action2 de producción de industrias (`GSF_INDUSTRIES`).
///
/// Las versiones 0/1 guardan los slots de entrada/salida implícitos; la
/// versión 2 añade el índice local de cada cargo. En versiones 1/2 los valores
/// de cantidad son índices de registro temporal (`7D`) y se resuelven al
/// ejecutar el callback.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IndustryProductionGroup {
    pub version: u8,
    pub subtract_input: Vec<i16>,
    pub cargo_input: Vec<u8>,
    pub add_output: Vec<u16>,
    pub cargo_output: Vec<u8>,
    pub again: u8,
}

impl Action2RealEntry {
    /// Select the set using the same proportional stage rule as
    /// `VehicleResolverObject::ResolveReal` in `OpenTTD`.
    #[must_use]
    pub fn selected_set(&self, loading: bool, cargo: u32, capacity: u32) -> Option<u16> {
        let sets = if loading { &self.loading } else { &self.loaded };
        let total = sets.len();
        if total == 0 {
            return None;
        }
        let denominator = capacity.max(1);
        let stage = usize::try_from(
            u64::from(cargo).saturating_mul(u64::try_from(total).unwrap_or(u64::MAX))
                / u64::from(denominator),
        )
        .unwrap_or(total.saturating_sub(1))
        .min(total.saturating_sub(1));
        sets.get(stage).copied()
    }
}

/// Ajuste `varadjust` (shift/and [+add+div|mod]).
///
/// The high bit of `shift` is kept as an internal parser marker for Action2
/// groups whose type selects the parent scope.  Only bits 0..4 are part of
/// the wire-format shift; [`Action2VarAdjust::shift_amount`] masks the marker
/// before applying the arithmetic.
pub(crate) const ACTION2_PARENT_SCOPE_MARKER: u8 = 0x80;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Action2VarAdjust {
    /// Bits 0..4 del `shift-num`.
    pub shift: u8,
    pub and_mask: u32,
    pub add_val: Option<u32>,
    pub divide_val: Option<u32>,
    pub modulo_val: Option<u32>,
}

impl Action2VarAdjust {
    /// Shift amount encoded by Action2 (`shift-num`, bits 0..4).
    #[must_use]
    pub const fn shift_amount(&self) -> u8 {
        self.shift & 0x1F
    }

    /// Whether this term was parsed from a parent-scope Action2 group.
    #[must_use]
    pub const fn is_parent_scope(&self) -> bool {
        self.shift & ACTION2_PARENT_SCOPE_MARKER != 0
    }
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
    pub ranges: Vec<(u16, u32, u32)>,
    pub default: u16,
}

/// Action2 random (`0x80`/`0x83`/`0x84`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action2RandomEntry {
    /// `0x80` propio, `0x83` related, `0x84` consist.
    pub typ: u8,
    /// Solo `0x84`: conteo desde vehículo de control (nibble bajo = offset).
    pub consist_count: u8,
    /// Bits 0..6: eventos; bit 7: comparar todos los eventos en vez de
    /// cualquiera (`Action2` raw `triggers`). Se conserva crudo para no perder
    /// la semántica `all` durante parse/reaplicación.
    pub triggers: u8,
    pub randbit: u8,
    pub sets: Vec<u16>,
}

impl Action2RandomEntry {
    /// Máscara de eventos Action2, sin el flag `all` del bit 7.
    #[must_use]
    pub const fn trigger_mask(&self) -> u8 {
        self.triggers & 0x7F
    }

    /// `true` si el bit 7 pide que todos los triggers del grupo estén activos.
    #[must_use]
    pub const fn requires_all_triggers(&self) -> bool {
        self.triggers & 0x80 != 0
    }

    /// Eventos consumidos cuando este grupo debe re-randomizarse.
    ///
    /// Devuelve `None` si el conjunto `waiting` no satisface la condición.
    /// En modo `all`, una máscara vacía es una condición válida, igual que
    /// `VarSpriteGroup::ResolveRerandomisation` de `OpenTTD`.
    #[must_use]
    pub const fn matched_rerandomisation_triggers(&self, waiting: u8) -> Option<u8> {
        let triggers = self.trigger_mask();
        let matched = triggers & waiting;
        if (self.requires_all_triggers() && matched == triggers)
            || (!self.requires_all_triggers() && matched != 0)
        {
            Some(matched)
        } else {
            None
        }
    }

    /// Bits que deben reseedearse si el grupo cambia de variante.
    #[must_use]
    pub fn rerandomisation_mask(&self) -> u32 {
        let width = u32::try_from(self.sets.len().saturating_sub(1)).unwrap_or(u32::MAX);
        width.checked_shl(u32::from(self.randbit)).unwrap_or(0)
    }
}

/// Contexto para evaluar variational / random (preview o runtime).
#[derive(Debug, Clone, Default)]
pub struct Action2EvalCtx {
    /// Valores de variables `NewGRF` (`variable` → raw).
    pub vars: HashMap<u8, u32>,
    /// Valores de variables que dependen de su parámetro `60+x`.
    ///
    /// La mayoría de scopes sólo necesita un valor por variable y usa
    /// [`Self::vars`]. Los scopes que consultan teselas vecinas, en cambio,
    /// pueden evaluar la misma variable con varios offsets dentro del mismo
    /// Action2 (por ejemplo `68[01]` y `68[0F]`). Esta tabla conserva esa
    /// distinción sin alterar las variables especiales `7C`–`7F`.
    pub parameterized_vars: HashMap<(u8, u8), u32>,
    /// Variables exposed by the parent scope of the resolved object.
    ///
    /// Action2 deterministic types `0x82`, `0x86` and `0x8A` select this
    /// table.  It is intentionally separate from [`Self::vars`], so a GRF
    /// can compare a child vehicle with its parent without overwriting the
    /// current unit's values.
    pub parent_vars: HashMap<u8, u32>,
    /// Parameterized variables available in the parent scope.
    pub parent_parameterized_vars: HashMap<(u8, u8), u32>,
    /// Bits aleatorios del objeto (vehículo/estación/…).
    pub random_bits: u32,
    /// Random bits of the parent scope (`0x83` random Action2).
    pub parent_random_bits: u32,
    /// Generación visual de la unidad; CB32 la incrementa cuando invalida la
    /// paleta y el renderer la incluye en la clave de caché.
    pub vehicle_palette_generation: u32,
    /// Generación visual del vehículo padre, cuando el scope parent participa
    /// en la selección de sprites.
    pub parent_vehicle_palette_generation: u32,
    /// Bits de vehículos del consist indexados por offset (`0x84` nibble bajo).
    pub consist_random_bits: HashMap<u8, u32>,
    /// Random bits indexed by signed relative position in a vehicle chain.
    /// Positive offsets move toward `next_unit` (away from the engine), and
    /// negative offsets move toward `prev_unit` (toward the engine).
    pub relative_random_bits: HashMap<i16, u32>,
    /// Random bits selected from the first contiguous vehicle run with the
    /// same engine id as the resolved vehicle. Action2 random type `0x84`
    /// direction `3` starts at that run and applies its count forward, just
    /// like `OpenTTD`'s `VehicleResolverObject::GetScope`.
    pub relative_same_engine_random_bits: HashMap<i16, u32>,
    /// Feature variables exposed by neighboring vehicles, indexed by
    /// `(signed_offset, variable)`.  This is the data source for Action2
    /// variable `61`, whose offset is selected through register `0x10F`.
    pub relative_vars: HashMap<(i16, u8), u32>,
    /// Parameterized counterpart of [`Self::relative_vars`].
    ///
    /// The secondary parameter is a WORD because vehicle variable `0x60`
    /// receives an `ExtendedByte` engine id through register `0x10E`.
    /// Keeping this wider than the ordinary `60+x` byte parameters prevents
    /// local ids above `0xFF` from aliasing the vanilla id zero.
    pub relative_parameterized_vars: HashMap<(i16, u8, u16), u32>,
    /// Registros temporales (variable `7D` / operador `\2sto`).
    pub temp_registers: HashMap<u8, u32>,
    /// Registros temporales extendidos (`0x100+`) escritos por `STO`.
    ///
    /// `OpenTTD` usa `0x100` para devolver el palette id y el bit 31 de
    /// continuidad de una secuencia `SpriteStack`.
    pub registers_100: HashMap<u16, u32>,
    /// Registros persistentes (variable `7C` / operador `\2psto`).
    pub persistent_registers: HashMap<u8, u32>,
    /// Persistent storage belonging to the parent scope, when that scope has
    /// one.  Generic register `7D` remains object-wide; `7C` is feature
    /// specific and may use this table for vehicle parent lookups.
    pub parent_persistent_registers: HashMap<u8, u32>,
    /// Último resultado de un `VarAction2` (variable `1C`; p. ej. tras procedure `7E`).
    pub last_result: u32,
    /// Parámetros del GRF (`GRFFile::param`; variable `0x7F[param]`).
    pub grf_params: Vec<u32>,
    /// Vehicle loading state used by Action2 real groups (`loaded`/`loading`).
    pub vehicle_loading: bool,
    /// Current cargo amount for proportional real-group selection.
    pub vehicle_cargo: u32,
    /// Vehicle cargo capacity for proportional real-group selection.
    pub vehicle_capacity: u32,
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
    /// Asignaciones Action3 cuyo id local usa `ExtendedByte` (WORD).
    /// Los IDs byte permanecen en [`Self::assigns`] para no romper features
    /// que históricamente sólo admitían 255 entradas.
    pub extended_assigns: Vec<(u16, u16)>,
    /// Action3 específico: `(id local, cargo/sprite-type)` → set Action2.
    ///
    /// Para `RailTypes` el segundo byte es `RailSpriteType`; señales usan `11`.
    pub specific_assigns: HashMap<(u8, u8), u16>,
    /// Equivalente extendido de `specific_assigns` para vehículos.
    pub extended_specific_assigns: HashMap<(u16, u8), u16>,
    /// Grupos Action3 que sustituyen el sprite de un vagón según el motor
    /// principal del consist (`SetWagonOverrideSprites` de `OpenTTD`).
    pub wagon_overrides: Vec<WagonOverrideAssign>,
    /// Action2 set-id → índice del primer set Action1 "moving" (solo trains).
    pub action2_to_action1: HashMap<u8, u16>,
    /// Action2 real groups with loaded/loading alternatives.
    pub action2_real: HashMap<u8, Action2RealEntry>,
    /// Action2 variational completo (rangos + default / advanced).
    pub action2_var: HashMap<u8, Action2VarEntry>,
    /// Action2 random (`0x80`/`0x83`/`0x84`).
    pub action2_random: HashMap<u8, Action2RandomEntry>,
    /// Action2 de producción para `GSF_INDUSTRIES` (versiones 0/1/2).
    pub industry_production: HashMap<u8, IndustryProductionGroup>,
    /// Grupos Action2 de tipo `TileLayoutSpriteGroup` (casas, objetos,
    /// aeropuertos, industrias y road stops). Se conserva el layout crudo para
    /// resolver sus sets Action1 cuando el feature se dibuja.
    pub tile_layouts: HashMap<u8, TileLayout>,
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
            if let Some(real) = self.action2_real.get(&a2)
                && let Some(next) =
                    real.selected_set(ctx.vehicle_loading, ctx.vehicle_cargo, ctx.vehicle_capacity)
            {
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

    /// Resuelve un layout `TileSeq` asignado a un id local. Los grupos
    /// variational/random intermedios siguen la misma cadena que los sprites
    /// simples; un grupo Action2 básico sin layout no se interpreta como un
    /// layout por accidente.
    pub fn tile_layout_for_local_id_ctx(
        &self,
        local_id: u16,
        view: usize,
        ctx: &mut Action2EvalCtx,
    ) -> Option<ResolvedTileLayout> {
        let mut id = self
            .extended_assigns
            .iter()
            .find(|(assigned, _)| *assigned == local_id)
            .map(|(_, set)| *set)
            .or_else(|| {
                u8::try_from(local_id).ok().and_then(|id| {
                    self.assigns
                        .iter()
                        .find(|assign| assign.local_id == id)
                        .map(|assign| assign.set_id)
                })
            })
            .or_else(|| (!self.sets.is_empty()).then_some(0))?;

        for _ in 0..8 {
            let a2 = u8::try_from(id).ok()?;
            if let Some(layout) = self.tile_layouts.get(&a2) {
                return Some(layout.resolve(self, view));
            }
            if let Some(random) = self.action2_random.get(&a2) {
                let next = eval_action2_random(random, ctx);
                if next & 0x8000 != 0 {
                    return None;
                }
                id = next;
                continue;
            }
            if let Some(real) = self.action2_real.get(&a2)
                && let Some(next) =
                    real.selected_set(ctx.vehicle_loading, ctx.vehicle_cargo, ctx.vehicle_capacity)
            {
                id = next;
                continue;
            }
            if let Some(var) = self.action2_var.get(&a2).cloned() {
                let next = eval_action2_var(self, &var, ctx, 0);
                if next & 0x8000 != 0 {
                    return None;
                }
                id = next;
                continue;
            }
            return None;
        }
        None
    }

    /// Recorre el camino Action3/Action2 activo y acumula los bits que deben
    /// re-randomizarse para `waiting_triggers`.
    ///
    /// Es la contraparte compacta de `ResolverObject::ResolveRerandomisation`:
    /// evalúa ramas variationales con el contexto presente, respeta random
    /// `any`/`all` y sólo visita la rama actualmente elegida por los bits
    /// anteriores al reseed. El resultado es `(máscara_de_bits, triggers_usados)`.
    #[must_use]
    pub fn rerandomisation_for_local_id(
        &self,
        local_id: u8,
        ctx: &mut Action2EvalCtx,
        waiting_triggers: u8,
    ) -> (u32, u8) {
        self.rerandomisation_for_local_id_u16(u16::from(local_id), ctx, waiting_triggers)
    }

    /// Variante de [`Self::rerandomisation_for_local_id`] para un ID local
    /// codificado como `ExtendedByte`.
    #[must_use]
    pub fn rerandomisation_for_local_id_u16(
        &self,
        local_id: u16,
        ctx: &mut Action2EvalCtx,
        waiting_triggers: u8,
    ) -> (u32, u8) {
        let start = self
            .extended_assigns
            .iter()
            .find(|(id, _)| *id == local_id)
            .map(|(_, set_id)| *set_id)
            .or_else(|| {
                self.assigns
                    .iter()
                    .find(|assign| u16::from(assign.local_id) == local_id)
                    .map(|assign| assign.set_id)
            })
            .or_else(|| (!self.sets.is_empty()).then_some(0));
        start.map_or((0, 0), |set_id| {
            self.rerandomisation_for_action2(set_id, ctx, waiting_triggers, 0)
        })
    }

    fn rerandomisation_for_action2(
        &self,
        set_id: u16,
        ctx: &mut Action2EvalCtx,
        waiting_triggers: u8,
        depth: u8,
    ) -> (u32, u8) {
        if depth >= 8 {
            return (0, 0);
        }
        let a2 = u8::try_from(set_id).unwrap_or(u8::MAX);
        if let Some(random) = self.action2_random.get(&a2) {
            let (mut reseed, mut used) = random
                .matched_rerandomisation_triggers(waiting_triggers)
                .map_or((0, 0), |matched| (random.rerandomisation_mask(), matched));
            let next = eval_action2_random(random, ctx);
            if next & 0x8000 == 0 {
                let (child_reseed, child_used) = self.rerandomisation_for_action2(
                    next,
                    ctx,
                    waiting_triggers,
                    depth.saturating_add(1),
                );
                reseed |= child_reseed;
                used |= child_used;
            }
            return (reseed, used);
        }
        if let Some(var) = self.action2_var.get(&a2).cloned() {
            let next = eval_action2_var(self, &var, ctx, depth);
            if next & 0x8000 == 0 {
                return self.rerandomisation_for_action2(
                    next,
                    ctx,
                    waiting_triggers,
                    depth.saturating_add(1),
                );
            }
        }
        (0, 0)
    }

    /// Todas las vistas del set asignado al id local (ctx por defecto).
    #[must_use]
    pub fn views_for_local_id(&self, local_id: u8) -> Option<&[DecodedSprite]> {
        self.views_for_local_id_u16(u16::from(local_id))
    }

    /// Todas las vistas para un ID local codificado como `ExtendedByte`.
    #[must_use]
    pub fn views_for_local_id_u16(&self, local_id: u16) -> Option<&[DecodedSprite]> {
        self.views_for_local_id_u16_ctx(local_id, &mut Action2EvalCtx::default())
    }

    /// Vistas resolviendo Action2 con contexto (random/consist/advanced).
    pub fn views_for_local_id_ctx(
        &self,
        local_id: u8,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        self.views_for_local_id_u16_ctx(u16::from(local_id), ctx)
    }

    /// Vistas resolviendo Action2 para un ID local extendido.
    pub fn views_for_local_id_u16_ctx(
        &self,
        local_id: u16,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        let set_id = self
            .extended_assigns
            .iter()
            .find(|(id, _)| *id == local_id)
            .map(|(_, set_id)| *set_id)
            .or_else(|| {
                self.assigns
                    .iter()
                    .find(|a| u16::from(a.local_id) == local_id)
                    .map(|a| a.set_id)
            })
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
        self.views_for_specific_u16_ctx(u16::from(local_id), selector, ctx)
    }

    /// Vistas del grupo Action3 específico para un ID local extendido.
    pub fn views_for_specific_u16_ctx(
        &self,
        local_id: u16,
        selector: u8,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        let set_id = self
            .extended_specific_assigns
            .get(&(local_id, selector))
            .copied()
            .or_else(|| {
                u8::try_from(local_id)
                    .ok()
                    .and_then(|id| self.specific_assigns.get(&(id, selector)).copied())
            })?;
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
        self.views_for_local_id_cargo_u16_ctx(u16::from(local_id), cargo, ctx)
    }

    /// Grupo específico de cargo para un ID local extendido.
    pub fn views_for_local_id_cargo_u16_ctx(
        &self,
        local_id: u16,
        cargo: Option<crate::cargo::CargoType>,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        if let Some(selector) = cargo.map(crate::cargo::CargoType::temperate_id)
            && self.has_specific_assignment_u16(local_id, selector)
        {
            return self.views_for_specific_u16_ctx(local_id, selector, ctx);
        }
        self.views_for_local_id_u16_ctx(local_id, ctx)
    }

    /// Vistas de un vagón aplicando un *wagon override* para el motor que
    /// encabeza el consist.
    ///
    /// `OpenTTD` recorre los overrides en orden de declaración y acepta primero
    /// una entrada específica del cargo o el grupo default (`0xFF`). Se
    /// conserva ese orden para que una cadena de GRF que declara varios
    /// overrides mantenga la misma precedencia.
    pub fn views_for_wagon_override_u16_ctx(
        &self,
        wagon_local_id: u16,
        overriding_local_id: u16,
        cargo: Option<crate::cargo::CargoType>,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&[DecodedSprite]> {
        let selector = cargo.map(crate::cargo::CargoType::temperate_id);
        let set_id = self
            .wagon_overrides
            .iter()
            .find(|override_assign| {
                override_assign.wagon_local_id == wagon_local_id
                    && override_assign.overriding_local_id == overriding_local_id
                    && (selector.is_some_and(|cargo_id| override_assign.selector == cargo_id)
                        || override_assign.selector == 0xFF)
            })
            .map(|override_assign| override_assign.set_id)?;
        let action1_idx = self.resolve_action1_set_ctx(set_id, ctx);
        self.sets
            .get(usize::from(action1_idx))
            .map(Vec::as_slice)
            .filter(|sprites| !sprites.is_empty())
    }

    /// ¿Action3 asignó este grupo específico al id local?
    #[must_use]
    pub fn has_specific_assignment(&self, local_id: u8, selector: u8) -> bool {
        self.has_specific_assignment_u16(u16::from(local_id), selector)
    }

    /// ¿Action3 asignó un grupo específico a un ID local extendido?
    #[must_use]
    pub fn has_specific_assignment_u16(&self, local_id: u16, selector: u8) -> bool {
        self.extended_specific_assigns
            .contains_key(&(local_id, selector))
            || u8::try_from(local_id)
                .is_ok_and(|id| self.specific_assigns.contains_key(&(id, selector)))
    }

    /// ¿Necesita re-resolución en runtime (random o cualquier variational)?
    #[must_use]
    pub fn needs_runtime_resolve(&self) -> bool {
        !self.action2_random.is_empty()
            || !self.action2_var.is_empty()
            || !self.action2_real.is_empty()
            || !self.industry_production.is_empty()
    }

    /// `TileLayout` también requiere conservar el grafo después de aplicar
    /// Action0: a diferencia de un sprite plano, el Action2 asignado contiene
    /// varias referencias y cajas `TILE_SEQ`.
    #[must_use]
    pub fn has_tile_layouts(&self) -> bool {
        !self.tile_layouts.is_empty()
    }

    /// Busca el grupo de producción asignado por Action3 y atraviesa grupos
    /// variational/random intermedios cuando el GRF los usa como selector.
    pub fn industry_production_group_u16(
        &self,
        local_id: u16,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&IndustryProductionGroup> {
        let mut id = self
            .extended_assigns
            .iter()
            .find(|(assigned, _)| *assigned == local_id)
            .map(|(_, set)| *set)
            .or_else(|| {
                u8::try_from(local_id).ok().and_then(|id| {
                    self.assigns
                        .iter()
                        .find(|assign| assign.local_id == id)
                        .map(|assign| assign.set_id)
                })
            })?;
        for _ in 0..8 {
            let a2 = u8::try_from(id).ok()?;
            if let Some(group) = self.industry_production.get(&a2) {
                return Some(group);
            }
            if let Some(random) = self.action2_random.get(&a2) {
                let next = eval_action2_random(random, ctx);
                if next & 0x8000 != 0 {
                    return None;
                }
                id = next;
                continue;
            }
            if let Some(var) = self.action2_var.get(&a2).cloned() {
                let next = eval_action2_var(self, &var, ctx, 0);
                if next & 0x8000 != 0 {
                    return None;
                }
                id = next;
                continue;
            }
            return None;
        }
        None
    }

    /// Variante byte para callers legacy que no usan IDs `ExtendedByte`.
    pub fn industry_production_group(
        &self,
        local_id: u8,
        ctx: &mut Action2EvalCtx,
    ) -> Option<&IndustryProductionGroup> {
        self.industry_production_group_u16(u16::from(local_id), ctx)
    }

    /// Resuelve un callback `NewGRF` (`nvar=0` → valor; sprite group → [`CALLBACK_FAILED`]).
    ///
    /// Inserta `0x0C`/`0x10`/`0x18` en el contexto (como `ResolverObject` upstream).
    #[must_use]
    pub fn resolve_callback(&self, local_id: u8, callback: u16, param1: u32, param2: u32) -> u16 {
        self.resolve_callback_u16(u16::from(local_id), callback, param1, param2)
    }

    /// Resuelve un callback para un ID local codificado como `ExtendedByte`.
    #[must_use]
    pub fn resolve_callback_u16(
        &self,
        local_id: u16,
        callback: u16,
        param1: u32,
        param2: u32,
    ) -> u16 {
        let mut ctx = Action2EvalCtx::default();
        self.resolve_callback_ctx_u16(local_id, callback, param1, param2, &mut ctx)
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
        self.resolve_callback_ctx_u16(u16::from(local_id), callback, param1, param2, ctx)
    }

    /// Como [`Self::resolve_callback_ctx`] para IDs locales extendidos.
    pub fn resolve_callback_ctx_u16(
        &self,
        local_id: u16,
        callback: u16,
        param1: u32,
        param2: u32,
        ctx: &mut Action2EvalCtx,
    ) -> u16 {
        let set_id = self
            .extended_assigns
            .iter()
            .find(|(id, _)| *id == local_id)
            .map(|(_, set_id)| *set_id)
            .or_else(|| {
                self.assigns
                    .iter()
                    .find(|a| u16::from(a.local_id) == local_id)
                    .map(|a| a.set_id)
            })
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
/// Callback vehículos: seleccionar humo/chispas y potencia visual (`0x10`).
pub const CBID_VEHICLE_VISUAL_EFFECT: u16 = 0x10;
/// Callback estaciones: layout de tesela al construir (`CBID_STATION_BUILD_TILE_LAYOUT`).
pub const CBID_STATION_BUILD_TILE_LAYOUT: u16 = 0x24;
/// Callback vehículos: acortar la longitud visual (`CBID_VEHICLE_LENGTH`).
pub const CBID_VEHICLE_LENGTH: u16 = 0x11;
/// Callback vehículos: ajustar la cantidad cargada por tick (`CBID_VEHICLE_LOAD_AMOUNT`).
pub const CBID_VEHICLE_LOAD_AMOUNT: u16 = 0x12;
/// Callback vehículos: añadir la siguiente parte articulada (`CBID_VEHICLE_ARTIC_ENGINE`).
pub const CBID_VEHICLE_ARTIC_ENGINE: u16 = 0x16;
/// Callback vehículos: capacidad efectiva después de un refit (`CBID_VEHICLE_REFIT_CAPACITY`).
pub const CBID_VEHICLE_REFIT_CAPACITY: u16 = 0x15;
/// Callback vehículos: permitir start/stop (`CBID_VEHICLE_START_STOP_CHECK`).
pub const CBID_VEHICLE_START_STOP_CHECK: u16 = 0x31;
/// Callback vehículos: invocado cada 32 días por vehículo (`CBID_VEHICLE_32DAY_CALLBACK`).
pub const CBID_VEHICLE_32DAY_CALLBACK: u16 = 0x32;
/// Callback vehículos: seleccionar un efecto de sonido (`CBID_VEHICLE_SOUND_EFFECT`).
pub const CBID_VEHICLE_SOUND_EFFECT: u16 = 0x33;
/// Callback vehículos: seleccionar el reemplazo automático (`CBID_VEHICLE_AUTOREPLACE_SELECTION`).
pub const CBID_VEHICLE_AUTOREPLACE_SELECTION: u16 = 0x34;
/// Callback vehículos: modificar una propiedad Action0 (`CBID_VEHICLE_MODIFY_PROPERTY`).
pub const CBID_VEHICLE_MODIFY_PROPERTY: u16 = 0x36;
/// Callback vehículos: seleccionar el mapa de colores (`CBID_VEHICLE_COLOUR_MAPPING`).
pub const CBID_VEHICLE_COLOUR_MAPPING: u16 = 0x2D;
/// Callback industrias: disponibilidad / ubicación al colocar (`CBID_INDUSTRY_LOCATION`).
pub const CBID_INDUSTRY_LOCATION: u16 = 0x28;
/// Callback industrias: cambio aleatorio de producción (`CBID_INDUSTRY_PRODUCTION_CHANGE`).
pub const CBID_INDUSTRY_PRODUCTION_CHANGE: u16 = 0x29;
/// Callback industrias: cambio mensual de producción (`CBID_INDUSTRY_MONTHLYPROD_CHANGE`).
pub const CBID_INDUSTRY_MONTHLY_PROD_CHANGE: u16 = 0x35;
/// Callback industrias: nivel inicial al fundar (`CBID_INDUSTRY_PROD_CHANGE_BUILD`).
pub const CBID_INDUSTRY_PROD_CHANGE_BUILD: u16 = 0x15F;
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
/// Callback estaciones: elegir layout de tesela al dibujar.
pub const CBID_STATION_DRAW_TILE_LAYOUT: u16 = 0x14;
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
