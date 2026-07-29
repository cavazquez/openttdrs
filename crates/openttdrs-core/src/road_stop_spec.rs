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
    /// Action0 `0x0C` draw modes (`RoadStopDrawModes`).
    #[serde(default = "default_road_stop_draw_mode")]
    pub draw_mode: u8,
    /// Action0 `0x12` flags DWORD (`RoadStopSpecFlags`).
    #[serde(default)]
    pub flags: u32,
    /// Vistas Action1/3 (opcional; no se serializan — se rehidratan al re-aplicar).
    #[serde(default, skip)]
    pub newgrf_views: Vec<crate::newgrf_sprites::DecodedSprite>,
    /// Ids de badges asociados (catálogo `badge`).
    #[serde(default)]
    pub associated_badges: Vec<u16>,
}

const fn default_road_stop_draw_mode() -> u8 {
    ROADSTOP_DRAW_MODE_DEFAULT
}

impl RoadStopSpecDef {
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
            draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
            flags,
            newgrf_views: Vec::new(),
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
                draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
                flags: 0,
                newgrf_views: Vec::new(),
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
                draw_mode: ROADSTOP_DRAW_MODE_DEFAULT,
                flags: 0,
                newgrf_views: Vec::new(),
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
