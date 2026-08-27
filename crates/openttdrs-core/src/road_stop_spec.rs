//! Clases y specs de paradas de carretera `NewGRF` (`RoadStops`, feature Action0 `0x14`).
//!
//! Catálogo runtime parcial: clase, tipo de parada, `flags/draw_mode` y nombre;
//! sprites opcionales vía Action1/3.

use serde::{Deserialize, Serialize};

/// Índice de vista bahía NE…NW / drive-through X/Y (`OpenTTD` `RoadStopView` / `RSV_*`).
pub const RSV_BAY_NE: u8 = 0;
pub const RSV_BAY_SE: u8 = 1;
pub const RSV_BAY_SW: u8 = 2;
pub const RSV_BAY_NW: u8 = 3;
/// Drive-through eje X (`GFX_TRUCK_BUS_DRIVETHROUGH_OFFSET + AXIS_X`).
pub const RSV_DRIVE_THROUGH_X: u8 = 4;
/// Drive-through eje Y.
pub const RSV_DRIVE_THROUGH_Y: u8 = 5;

/// `RoadStopAvailabilityType` (`OpenTTD` `newgrf_roadstop.h`).
pub const ROADSTOP_TYPE_BUS: u8 = 0;
pub const ROADSTOP_TYPE_TRUCK: u8 = 1;
pub const ROADSTOP_TYPE_ALL: u8 = 2;

/// Bits de `RoadStopSpecFlag` (`OpenTTD` `newgrf_roadstop.h`; Action0 prop `0x12` DWORD).
///
/// | Bit | Flag | Semántica |
/// |----:|------|-----------|
/// | 0 | `Cb141RandomBits` | Callback 141 necesita bits aleatorios |
/// | 2 | `NoCatenary` | No dibujar catenaria |
/// | 3 | `DriveThroughOnly` | Solo drive-through (rechaza bahía) |
/// | 4 | `NoAutoRoadConnection` | Sin auto-conexión de carretera |
/// | 5 | `RoadOnly` | Solo menú/carretera (no tranvía) |
/// | 6 | `TramOnly` | Solo menú/tranvía (no carretera) |
/// | 8 | `DrawModeRegister` | Leer draw mode del registro `0x100` |
pub const ROADSTOP_FLAG_CB141_RANDOM_BITS: u32 = 1 << 0;
/// `RoadStopSpecFlag::NoCatenary`: no emitir postes ni cables sobre la parada.
pub const ROADSTOP_FLAG_NO_CATENARY: u32 = 1 << 2;
pub const ROADSTOP_FLAG_DRIVE_THROUGH_ONLY: u32 = 1 << 3;
pub const ROADSTOP_FLAG_ROAD_ONLY: u32 = 1 << 5;
pub const ROADSTOP_FLAG_TRAM_ONLY: u32 = 1 << 6;

/// Bits de `RoadStopDrawMode` (Action0 prop `0x0C` BYTE).
///
/// | Bit | Modo | Semántica |
/// |----:|------|-----------|
/// | 0 | `Road` | Bahía: dibujar la carretera |
/// | 1 | `Overlay` | Drive-through: overlay (p. ej. acera) |
/// | 2 | `WaypGround` | Waypoint: suelo del layout |
pub const ROADSTOP_DRAW_MODE_ROAD: u8 = 1 << 0;
pub const ROADSTOP_DRAW_MODE_OVERLAY: u8 = 1 << 1;
/// Default OTTD: `Road | Overlay`.
pub const ROADSTOP_DRAW_MODE_DEFAULT: u8 = ROADSTOP_DRAW_MODE_ROAD | ROADSTOP_DRAW_MODE_OVERLAY;

/// Bits de `RoadStopCallbackMask` (Action0 prop `0x11`).
///
/// El bit de disponibilidad usa `CBID_STATION_AVAILABILITY` (`0x13`) tanto
/// en el picker como antes de ejecutar la construcción, igual que
/// `RoadStopChangeInfo` / `CmdBuildRoadStop` de `OpenTTD`.
pub const ROADSTOP_CALLBACK_MASK_AVAILABILITY: u8 = 1 << 0;
/// `CBID_STATION_ANIMATION_NEXT_FRAME` (`0x141`) personaliza el scheduler.
pub const ROADSTOP_CALLBACK_MASK_ANIMATION_NEXT_FRAME: u8 = 1 << 1;
/// `CBID_STATION_ANIMATION_SPEED` (`0x142`) personaliza su espera entre frames.
pub const ROADSTOP_CALLBACK_MASK_ANIMATION_SPEED: u8 = 1 << 2;

/// Triggers de animación de una parada (`StationAnimationTrigger`).
///
/// Action0 guarda la máscara, pero CB140 recibe el ordinal correspondiente.
/// Mantener estas constantes ligadas al enum compartido evita que ambas
/// representaciones se desalineen.
pub const ROADSTOP_ANIMATION_TRIGGER_BUILT: u16 = crate::StationAnimationTrigger::Built.mask();
pub const ROADSTOP_ANIMATION_TRIGGER_NEW_CARGO: u16 =
    crate::StationAnimationTrigger::NewCargo.mask();
pub const ROADSTOP_ANIMATION_TRIGGER_CARGO_TAKEN: u16 =
    crate::StationAnimationTrigger::CargoTaken.mask();
pub const ROADSTOP_ANIMATION_TRIGGER_VEHICLE_ARRIVES: u16 =
    crate::StationAnimationTrigger::VehicleArrives.mask();
pub const ROADSTOP_ANIMATION_TRIGGER_VEHICLE_DEPARTS: u16 =
    crate::StationAnimationTrigger::VehicleDeparts.mask();
pub const ROADSTOP_ANIMATION_TRIGGER_VEHICLE_LOADS: u16 =
    crate::StationAnimationTrigger::VehicleLoads.mask();
pub const ROADSTOP_ANIMATION_TRIGGER_ACCEPTANCE_TICK: u16 =
    crate::StationAnimationTrigger::AcceptanceTick.mask();
pub const ROADSTOP_ANIMATION_TRIGGER_TILE_LOOP: u16 =
    crate::StationAnimationTrigger::TileLoop.mask();

/// Metadatos de una clase de road stop (`RoadStopClass`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadStopClassDef {
    pub id: u16,
    pub label: String,
    pub short_label: String,
    pub from_newgrf: bool,
}

/// Spec de road stop (`RoadStopSpec` simplificado).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadStopSpecDef {
    pub id: u16,
    pub class: u16,
    pub label: String,
    pub short_label: String,
    /// `0` bus, `1` truck, `2` ambos (`RoadStopAvailabilityType`).
    pub stop_type: u8,
    pub from_newgrf: bool,
    /// GRFID del `NewGRF` que definió este spec (`0` = vanilla / sin set).
    #[serde(default)]
    pub grfid: u32,
    /// Id local Action0/Action3 en el GRF (identidad estable multi-GRF / save).
    #[serde(default)]
    pub newgrf_local_id: u8,
    /// Versión de formato de Action8 del GRF dueño. Determina cómo se
    /// traduce la máscara de cargos Action0 `0x0D` sin CTT explícita.
    #[serde(default)]
    pub newgrf_grf_version: u8,
    /// Action0 `0x0C` draw modes (`RoadStopDrawModes`).
    #[serde(default = "default_road_stop_draw_mode")]
    pub draw_mode: u8,
    /// Action0 `0x0D`: máscara de slots de cargo locales que disparan la
    /// randomización Action2. Se traduce al consultar, respetando CTT /
    /// versión del GRF como `TranslateRefitMask` de `OpenTTD`.
    #[serde(default)]
    pub random_cargo_triggers: u32,
    /// Action0 `0x12` flags DWORD (`RoadStopSpecFlags`).
    #[serde(default)]
    pub flags: u32,
    /// Action0 `0x11` (`RoadStopCallbackMask`). El bit de disponibilidad se
    /// ejecuta al previsualizar y construir; los bits 1/2 accionan el
    /// scheduler de animación de la parada.
    #[serde(default)]
    pub callback_mask: u8,
    /// Action0 `0x0E`: último frame y estado (`0` no-loop, `1` loop,
    /// `0xFF` sin animación).
    #[serde(default = "default_road_stop_animation_status")]
    pub animation_status: u8,
    /// Action0 `0x0E`: último frame de la animación.
    #[serde(default)]
    pub animation_frames: u8,
    /// Action0 `0x0F`: espera en ticks `2^speed`.
    #[serde(default = "default_road_stop_animation_speed")]
    pub animation_speed: u8,
    /// Action0 `0x10`: máscara `StationAnimationTrigger`.
    #[serde(default)]
    pub animation_triggers: u16,
    /// Vistas Action1/3 (opcional; no se serializan — se rehidratan al re-aplicar).
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Grafo Action2 para callbacks / vistas dinámicas. No se serializa: se
    /// rehidrata desde el stack `NewGRF` al cargar la partida.
    #[serde(default, skip)]
    pub newgrf_runtime: Option<Box<crate::newgrf_sprites::TrainSpriteGraphics>>,
    /// Tablas Action0 `GlobalVar` del GRF para traducir la carretera/tranvía en
    /// las variables `0x43`/`0x44` de callbacks.
    #[serde(default, skip)]
    pub newgrf_type_tables: Option<crate::newgrf_type_tables::GrfTypeTranslationTables>,
    /// Ids de badges asociados (catálogo `badge`).
    #[serde(default)]
    pub associated_badges: Vec<u16>,
}

const fn default_road_stop_draw_mode() -> u8 {
    ROADSTOP_DRAW_MODE_DEFAULT
}

const fn default_road_stop_animation_status() -> u8 {
    0xFF
}

const fn default_road_stop_animation_speed() -> u8 {
    2
}

impl RoadStopSpecDef {
    /// Id local del cargo para CB140 (`var 18`, bits 8..15).
    ///
    /// Los catálogos antiguos que no conservaron Action8 usan `0`, que
    /// `local_cargo_id` trata como formato moderno seguro (bitnum global).
    #[must_use]
    pub fn newgrf_cargo_local_id(&self, cargo: crate::CargoType, climate: crate::Climate) -> u8 {
        crate::newgrf_type_tables::local_cargo_id(
            self.newgrf_type_tables.as_ref(),
            self.newgrf_grf_version,
            cargo,
            climate,
        )
    }

    /// `true` si este cargo dispara la re-randomización Action2 de la parada.
    #[must_use]
    pub fn cargo_triggers_randomisation(
        &self,
        cargo: crate::CargoType,
        climate: crate::Climate,
    ) -> bool {
        let local_id = self.newgrf_cargo_local_id(cargo, climate);
        local_id < 32 && self.random_cargo_triggers & (1_u32 << local_id) != 0
    }

    /// `true` si el spec declaró al menos un cargo que habilita randomización.
    #[must_use]
    pub const fn has_random_cargo_triggers(&self) -> bool {
        self.random_cargo_triggers != 0
    }

    /// Vista Action1/3 para índice de gfx (`RSV_*`).
    ///
    /// Bahía (`0..3`): módulo si hay vistas. Drive-through (`4`/`5`): solo si
    /// `newgrf_views.len() > idx`; si no, `None` → Action5 / `OpenGFX`.
    #[must_use]
    pub fn newgrf_view(&self, idx: usize) -> Option<&crate::newgrf_sprites::DecodedSprite> {
        if self.newgrf_views.is_empty() {
            return None;
        }
        if idx >= 4 {
            return self.newgrf_views.get(idx);
        }
        self.newgrf_views.get(idx % self.newgrf_views.len())
    }

    /// Vista re-resolviendo Action2 con el contexto runtime de la parada.
    pub fn newgrf_view_runtime(
        &self,
        idx: usize,
        ctx: &mut crate::newgrf_sprites::Action2EvalCtx,
    ) -> Option<crate::newgrf_sprites::DecodedSprite> {
        let runtime = self.newgrf_runtime.as_ref()?;
        let views = runtime.views_for_local_id_ctx(self.newgrf_local_id, ctx)?;
        if views.is_empty() {
            return None;
        }
        Some(views[idx % views.len()].clone())
    }

    /// `true` si el `stop_type` coincide con la clase de parada a construir.
    #[must_use]
    pub fn matches_stop_kind(&self, kind: crate::station::StopKind) -> bool {
        if self.stop_type == ROADSTOP_TYPE_ALL {
            return matches!(
                kind,
                crate::station::StopKind::BusStop | crate::station::StopKind::TruckStop
            );
        }
        match kind {
            crate::station::StopKind::BusStop => self.stop_type == ROADSTOP_TYPE_BUS,
            crate::station::StopKind::TruckStop => self.stop_type == ROADSTOP_TYPE_TRUCK,
            _ => false,
        }
    }

    #[must_use]
    pub const fn drive_through_only(&self) -> bool {
        self.flags & ROADSTOP_FLAG_DRIVE_THROUGH_ONLY != 0
    }

    #[must_use]
    pub const fn road_only(&self) -> bool {
        self.flags & ROADSTOP_FLAG_ROAD_ONLY != 0
    }

    #[must_use]
    pub const fn tram_only(&self) -> bool {
        self.flags & ROADSTOP_FLAG_TRAM_ONLY != 0
    }

    /// `true` si el GRF declaró `CBID_STATION_AVAILABILITY` (`0x13`).
    #[must_use]
    pub const fn has_availability_callback(&self) -> bool {
        self.callback_mask & ROADSTOP_CALLBACK_MASK_AVAILABILITY != 0
    }

    /// `CBID_STATION_ANIMATION_NEXT_FRAME` (`0x141`) está habilitado.
    #[must_use]
    pub const fn has_animation_next_frame_callback(&self) -> bool {
        self.callback_mask & ROADSTOP_CALLBACK_MASK_ANIMATION_NEXT_FRAME != 0
    }

    /// `CBID_STATION_ANIMATION_SPEED` (`0x142`) está habilitado.
    #[must_use]
    pub const fn has_animation_speed_callback(&self) -> bool {
        self.callback_mask & ROADSTOP_CALLBACK_MASK_ANIMATION_SPEED != 0
    }

    /// `CB141` recibe random bits en `param1`.
    #[must_use]
    pub const fn animation_next_frame_uses_random_bits(&self) -> bool {
        self.flags & ROADSTOP_FLAG_CB141_RANDOM_BITS != 0
    }

    /// La propiedad `0x0E` declara una secuencia circular.
    #[must_use]
    pub const fn animation_loops(&self) -> bool {
        self.animation_status == 1
    }
}

/// `true` si `orientation` es drive-through X/Y (`RSV_DRIVE_THROUGH_*`).
#[must_use]
pub const fn is_drive_through_orientation(orientation: u8) -> bool {
    orientation == RSV_DRIVE_THROUGH_X || orientation == RSV_DRIVE_THROUGH_Y
}

/// Eje Y del drive-through (`orientation == 5`).
#[must_use]
pub const fn drive_through_axis_y(orientation: u8) -> bool {
    orientation == RSV_DRIVE_THROUGH_Y
}

/// Catálogo vacío de clases (solo desde `NewGRF`).
#[must_use]
pub fn empty_road_stop_class_catalog() -> Vec<RoadStopClassDef> {
    Vec::new()
}

/// Catálogo vacío de specs (solo desde `NewGRF`).
#[must_use]
pub fn empty_road_stop_spec_catalog() -> Vec<RoadStopSpecDef> {
    Vec::new()
}

/// Siguiente id libre de clase.
#[must_use]
pub fn next_free_road_stop_class_id(catalog: &[RoadStopClassDef]) -> Option<u16> {
    (0u16..u16::MAX).find(|&id| !catalog.iter().any(|d| d.id == id))
}

/// Siguiente id libre de spec.
#[must_use]
pub fn next_free_road_stop_spec_id(catalog: &[RoadStopSpecDef]) -> Option<u16> {
    (0u16..u16::MAX).find(|&id| !catalog.iter().any(|d| d.id == id))
}

#[must_use]
pub fn road_stop_class_def(catalog: &[RoadStopClassDef], id: u16) -> Option<&RoadStopClassDef> {
    catalog.iter().find(|d| d.id == id)
}

#[must_use]
pub fn road_stop_spec_def(catalog: &[RoadStopSpecDef], id: u16) -> Option<&RoadStopSpecDef> {
    catalog.iter().find(|d| d.id == id)
}

/// Busca spec por identidad estable `(grfid, newgrf_local_id)`.
#[must_use]
pub fn road_stop_spec_by_grf_local(
    catalog: &[RoadStopSpecDef],
    grfid: u32,
    local_id: u8,
) -> Option<&RoadStopSpecDef> {
    catalog
        .iter()
        .find(|d| d.from_newgrf && d.grfid == grfid && d.newgrf_local_id == local_id)
}

/// Primer spec `from_newgrf` compatible con `kind` (auto-select S-slice).
#[must_use]
pub fn first_matching_road_stop_spec(
    catalog: &[RoadStopSpecDef],
    kind: crate::station::StopKind,
) -> Option<&RoadStopSpecDef> {
    catalog
        .iter()
        .find(|d| d.from_newgrf && d.matches_stop_kind(kind))
}

/// Clases con al menos un spec compatible con `kind`.
#[must_use]
pub fn list_road_stop_classes<'a>(
    classes: &'a [RoadStopClassDef],
    specs: &[RoadStopSpecDef],
    kind: crate::station::StopKind,
) -> Vec<&'a RoadStopClassDef> {
    classes
        .iter()
        .filter(|c| {
            specs
                .iter()
                .any(|s| s.class == c.id && s.matches_stop_kind(kind))
        })
        .collect()
}

/// Specs de una clase (o todas si `class` es `None`) filtrados por `kind`.
#[must_use]
pub fn list_road_stop_specs(
    specs: &[RoadStopSpecDef],
    class: Option<u16>,
    kind: crate::station::StopKind,
) -> Vec<&RoadStopSpecDef> {
    specs
        .iter()
        .filter(|s| s.matches_stop_kind(kind))
        .filter(|s| class.is_none_or(|c| s.class == c))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::station::StopKind;

    fn sample_spec(stop_type: u8, flags: u32) -> RoadStopSpecDef {
        RoadStopSpecDef {
            id: 1,
            class: 0,
            label: "T".into(),
            short_label: "T".into(),
            stop_type,
            from_newgrf: true,
            grfid: 1,
            newgrf_local_id: 0,
            newgrf_grf_version: 0,
            draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
            random_cargo_triggers: 0,
            flags,
            callback_mask: 0,
            animation_status: 0xFF,
            animation_frames: 0,
            animation_speed: 2,
            animation_triggers: 0,
            newgrf_views: Vec::new(),
            newgrf_runtime: None,
            newgrf_type_tables: None,
            associated_badges: Vec::new(),
        }
    }

    fn sample_catalog() -> (Vec<RoadStopClassDef>, Vec<RoadStopSpecDef>) {
        let classes = vec![
            RoadStopClassDef {
                id: 0,
                label: "Bus".into(),
                short_label: "BUS".into(),
                from_newgrf: true,
            },
            RoadStopClassDef {
                id: 1,
                label: "Truck".into(),
                short_label: "TRK".into(),
                from_newgrf: true,
            },
        ];
        let specs = vec![
            RoadStopSpecDef {
                id: 10,
                class: 0,
                label: "Bus A".into(),
                short_label: "BA".into(),
                stop_type: ROADSTOP_TYPE_BUS,
                from_newgrf: true,
                grfid: 0,
                newgrf_local_id: 0,
                newgrf_grf_version: 0,
                draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
                random_cargo_triggers: 0,
                flags: 0,
                callback_mask: 0,
                animation_status: 0xFF,
                animation_frames: 0,
                animation_speed: 2,
                animation_triggers: 0,
                newgrf_views: Vec::new(),
                newgrf_runtime: None,
                newgrf_type_tables: None,
                associated_badges: Vec::new(),
            },
            RoadStopSpecDef {
                id: 11,
                class: 1,
                label: "Truck A".into(),
                short_label: "TA".into(),
                stop_type: ROADSTOP_TYPE_TRUCK,
                from_newgrf: true,
                grfid: 0,
                newgrf_local_id: 0,
                newgrf_grf_version: 0,
                draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
                random_cargo_triggers: 0,
                flags: 0,
                callback_mask: 0,
                animation_status: 0xFF,
                animation_frames: 0,
                animation_speed: 2,
                animation_triggers: 0,
                newgrf_views: Vec::new(),
                newgrf_runtime: None,
                newgrf_type_tables: None,
                associated_badges: Vec::new(),
            },
        ];
        (classes, specs)
    }

    #[test]
    fn list_helpers_filter_by_stop_kind() {
        let (classes, specs) = sample_catalog();
        let bus_classes = list_road_stop_classes(&classes, &specs, StopKind::BusStop);
        assert_eq!(bus_classes.len(), 1);
        assert_eq!(bus_classes[0].id, 0);
        let truck_specs = list_road_stop_specs(&specs, Some(1), StopKind::TruckStop);
        assert_eq!(truck_specs.len(), 1);
        assert_eq!(truck_specs[0].id, 11);
        assert!(list_road_stop_specs(&specs, Some(0), StopKind::TruckStop).is_empty());
    }

    #[test]
    fn stop_type_all_matches_bus_and_truck() {
        let def = sample_spec(ROADSTOP_TYPE_ALL, 0);
        assert!(def.matches_stop_kind(StopKind::BusStop));
        assert!(def.matches_stop_kind(StopKind::TruckStop));
    }

    #[test]
    fn random_cargo_trigger_uses_ctt_and_grf_version() {
        let mut def = sample_spec(ROADSTOP_TYPE_BUS, 0);
        def.newgrf_grf_version = 8;
        def.newgrf_type_tables = Some(crate::newgrf_type_tables::GrfTypeTranslationTables {
            cargo: vec![*b"MAIL", *b"GOOD", *b"PASS"],
            ..Default::default()
        });
        def.random_cargo_triggers = 1 << 1; // GOOD ocupa el slot local 1.
        assert!(
            def.cargo_triggers_randomisation(crate::CargoType::Goods, crate::Climate::Temperate)
        );
        assert!(
            !def.cargo_triggers_randomisation(
                crate::CargoType::Passengers,
                crate::Climate::Temperate
            )
        );

        // Sin CTT, un GRF antiguo traduce por el slot de clima (Paper=9 en
        // SubArctic), no por el bitnum global de Paper (11).
        def.newgrf_type_tables = None;
        def.newgrf_grf_version = 6;
        def.random_cargo_triggers = 1 << 9;
        assert!(
            def.cargo_triggers_randomisation(crate::CargoType::Paper, crate::Climate::SubArctic)
        );
    }

    #[test]
    fn runtime_view_uses_road_stop_random_bits() {
        use crate::newgrf_sprites::{
            Action2EvalCtx, Action2RandomEntry, TrainSpriteAssign, TrainSpriteGraphics,
        };

        fn solid(r: u8, g: u8, b: u8) -> crate::DecodedSprite {
            crate::DecodedSprite {
                width: 1,
                height: 1,
                x_offs: 0,
                y_offs: 0,
                rgba: vec![r, g, b, 255],
                mask: Vec::new(),
            }
        }

        let mut gfx = TrainSpriteGraphics {
            sets: vec![vec![solid(255, 0, 0)], vec![solid(0, 0, 255)]],
            ..Default::default()
        };
        gfx.assigns.push(TrainSpriteAssign {
            local_id: 0,
            set_id: 7,
        });
        gfx.action2_random.insert(
            7,
            Action2RandomEntry {
                typ: 0x80,
                consist_count: 0,
                triggers: 0,
                randbit: 0,
                sets: vec![0, 1],
            },
        );
        let mut def = sample_spec(ROADSTOP_TYPE_BUS, 0);
        def.newgrf_runtime = Some(Box::new(gfx));

        let mut zero = Action2EvalCtx::default();
        let mut one = Action2EvalCtx {
            random_bits: 1,
            ..Default::default()
        };
        assert_eq!(def.newgrf_view_runtime(0, &mut zero).unwrap().rgba[0], 255);
        assert_eq!(def.newgrf_view_runtime(0, &mut one).unwrap().rgba[2], 255);
    }

    #[test]
    fn dt_view_requires_enough_sprites() {
        let sprite = crate::newgrf_sprites::DecodedSprite {
            width: 1,
            height: 1,
            x_offs: 0,
            y_offs: 0,
            rgba: vec![0, 0, 0, 0],
            mask: Vec::new(),
        };
        let mut def = sample_spec(0, 0);
        def.newgrf_views = vec![sprite.clone(); 4];
        assert!(def.newgrf_view(0).is_some());
        assert!(def.newgrf_view(4).is_none());
        def.newgrf_views.push(sprite);
        assert!(def.newgrf_view(4).is_some());
    }

    #[test]
    fn flag_helpers() {
        let def = sample_spec(
            0,
            ROADSTOP_FLAG_DRIVE_THROUGH_ONLY | ROADSTOP_FLAG_ROAD_ONLY,
        );
        assert!(def.drive_through_only());
        assert!(def.road_only());
        assert!(!def.tram_only());
    }
}
