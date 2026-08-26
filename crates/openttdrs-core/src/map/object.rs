//! Objetos de mapa (`MP_OBJECT` / `TileType::Object` en `OpenTTD`).

use super::{Map, Tile, TileCoord};
use crate::newgrf_sprites::Action2EvalCtx;
use crate::object_spec::{
    NEW_OBJECT_OFFSET, ObjectSpecDef, decode_object_tile_offset, encode_object_tile_offset,
    object_footprint_tile_index, object_spec_def,
};
use crate::world_gen::Climate;

/// Nibble alto de `mapt` para teselas objeto.
pub const OTTD_MP_OBJECT: u8 = 10;

/// `mapt` con tipo objeto (bits 4–7 = 10).
pub const MP_OBJECT_MAPT: u8 = OTTD_MP_OBJECT << 4;

/// `ObjectType` en saves vanilla (`object_type.h`).
pub const OBJECT_TYPE_TRANSMITTER: u8 = 0;
pub const OBJECT_TYPE_LIGHTHOUSE: u8 = 1;
/// Estatua de compañía construida por la autoridad local (`SPR_STATUE_COMPANY`).
pub const OBJECT_TYPE_STATUE_COMPANY: u8 = 2;
pub const OBJECT_TYPE_OWNED_LAND: u8 = 3;

#[must_use]
pub const fn is_map_object_tile(mapt: u8) -> bool {
    (mapt >> 4) & 0xF == OTTD_MP_OBJECT
}

/// `true` si un tipo de objeto es un id `NewGRF` (`≥` [`NEW_OBJECT_OFFSET`]).
#[must_use]
pub const fn is_newgrf_object_type_id(object_type: u16) -> bool {
    object_type >= NEW_OBJECT_OFFSET
}

/// Compatibilidad para mapas locales históricos que guardan el tipo en `m5`.
#[must_use]
pub const fn is_newgrf_object_type(m5: u8) -> bool {
    is_newgrf_object_type_id(m5 as u16)
}

/// `ObjectID` crudo de una tesela `MP_OBJECT`.
///
/// OpenTTD lo forma como `m2() | (m5() << 16)`. El tipo visual se consulta en
/// el pool `OBJS`, no en `m5`.
#[must_use]
pub const fn object_id_from_tile(tile: &Tile) -> Option<u32> {
    if is_map_object_tile(tile.mapt) {
        Some((tile.m2 as u32) | ((tile.m2_hi as u32) << 8) | ((tile.m5 as u32) << 16))
    } else {
        None
    }
}

/// Tipo codificado localmente en `m5`.
///
/// Los mapas creados por este proyecto conservaron este layout antes de que el
/// importador recibiera el pool `OBJS`. Para una partida OpenTTD importada usar
/// [`Map::object_type_at`], que consulta el `ObjectID` y el footer `OBTY`.
#[must_use]
pub const fn object_type_from_tile(tile: &Tile) -> Option<u8> {
    if is_map_object_tile(tile.mapt) {
        Some(tile.m5)
    } else {
        None
    }
}

impl Map {
    /// Instala el pool `ObjectID → ObjectType` leído del footer `OBTY`.
    ///
    /// Incluso un pool vacío es significativo: desactiva el fallback histórico
    /// que interpreta `m5` como tipo y por tanto preserva la semántica cruda de
    /// un save OpenTTD moderno.
    pub(crate) fn set_imported_object_types_from_footer(&mut self, types: &[(u32, u16)]) {
        self.imported_object_types = Some(types.iter().copied().collect());
    }

    /// Tipo visual efectivo de una tesela `MP_OBJECT`.
    ///
    /// En imports modernos usa el pool `OBJS` transportado por `OBTY`; para
    /// mapas locales o exports históricos sin ese footer mantiene el formato
    /// previo donde el tipo estaba en `m5`.
    #[must_use]
    pub fn object_type_at(&self, at: TileCoord) -> Option<u16> {
        let tile = self.get(at)?;
        let object_id = object_id_from_tile(&tile)?;
        match &self.imported_object_types {
            Some(types) => types.get(&object_id).copied(),
            None => Some(u16::from(tile.m5)),
        }
    }
}

/// Id de spec `NewGRF` persistido en `m5`, si aplica.
#[must_use]
pub const fn object_spec_id_from_tile(tile: &Tile) -> Option<u16> {
    match object_type_from_tile(tile) {
        Some(m5) if is_newgrf_object_type(m5) => Some(m5 as u16),
        _ => None,
    }
}

#[must_use]
pub const fn is_owned_land_tile(tile: &Tile) -> bool {
    is_map_object_tile(tile.mapt) && tile.m5 == OBJECT_TYPE_OWNED_LAND
}

/// Dimensiones W×H del objeto (vanilla = 1×1).
#[must_use]
pub fn object_type_dims(object_type: u8, catalog: &[ObjectSpecDef]) -> (u8, u8) {
    object_type_dims_id(u16::from(object_type), catalog)
}

/// Dimensiones W×H de un `ObjectType` completo (incluye IDs NewGRF > 255).
#[must_use]
pub fn object_type_dims_id(object_type: u16, catalog: &[ObjectSpecDef]) -> (u8, u8) {
    if is_newgrf_object_type_id(object_type) {
        object_spec_def(catalog, object_type).map_or((1, 1), |d| (d.size_width(), d.size_height()))
    } else {
        (1, 1)
    }
}

/// Teselas del footprint con origen en `origin` y tamaño `w`×`h`.
#[must_use]
pub fn object_footprint_tiles(origin: TileCoord, w: u8, h: u8) -> Vec<TileCoord> {
    let mut out = Vec::with_capacity(usize::from(w).saturating_mul(usize::from(h)));
    for dy in 0..h {
        for dx in 0..w {
            out.push(TileCoord::new(
                origin.x.saturating_add(i32::from(dx)),
                origin.y.saturating_add(i32::from(dy)),
            ));
        }
    }
    out
}

/// Origen del objeto a partir de una tesela del footprint (`m2` = offset).
#[must_use]
pub fn object_origin_from_tile(tile: &Tile, at: TileCoord) -> Option<TileCoord> {
    let _ = object_type_from_tile(tile)?;
    let (dx, dy) = decode_object_tile_offset(tile.m2);
    Some(TileCoord::new(
        at.x.saturating_sub(i32::from(dx)),
        at.y.saturating_sub(i32::from(dy)),
    ))
}

/// Todas las teselas del footprint del objeto que contiene `at`.
#[must_use]
pub fn object_footprint_at(
    map: &Map,
    at: TileCoord,
    catalog: &[ObjectSpecDef],
) -> Option<Vec<TileCoord>> {
    let tile = map.get(at)?;
    let object_type = map.object_type_at(at)?;
    let origin = object_origin_from_tile(&tile, at)?;
    let (w, h) = object_type_dims_id(object_type, catalog);
    Some(object_footprint_tiles(origin, w, h))
}

/// Índice de vista Action3 para la tesela (`dy * width + dx`).
#[must_use]
pub fn object_view_index_for_tile(tile: &Tile, catalog: &[ObjectSpecDef]) -> Option<usize> {
    let object_type = object_type_from_tile(tile)?;
    object_view_index_for_type(tile, u16::from(object_type), catalog)
}

/// Índice de vista Action3 para una tesela y su `ObjectType` ya resuelto.
#[must_use]
pub fn object_view_index_for_type(
    tile: &Tile,
    object_type: u16,
    catalog: &[ObjectSpecDef],
) -> Option<usize> {
    object_id_from_tile(tile)?;
    let (w, _) = object_type_dims_id(object_type, catalog);
    let (dx, dy) = decode_object_tile_offset(tile.m2);
    Some(object_footprint_tile_index(dx, dy, w))
}

/// Codifica offset de footprint en `m2` (reexport útil para build).
#[must_use]
pub const fn object_tile_offset_byte(dx: u8, dy: u8) -> u8 {
    encode_object_tile_offset(dx, dy)
}

/// Construye el contexto de variables Action2 para una tesela de objeto.
///
/// Es la traducción del `ObjectScopeResolver` de OpenTTD para los datos que
/// están presentes en nuestro mapa: `m3` contiene los bits aleatorios del
/// objeto, `m2` conserva el offset dentro del footprint local, `m3hi` es el
/// `m4` de animación y `m1` el propietario. Las variables que requieren el
/// objeto/town global (fecha de construcción, pueblo más cercano, color,
/// teselas vecinas y conteos) quedan deliberadamente ausentes; el evaluador
/// las trata como no disponibles en vez de inventar valores.
#[must_use]
pub fn action2_eval_ctx_for_object_tile(tile: Tile, tileh: u8, climate: Climate) -> Action2EvalCtx {
    let mut ctx = Action2EvalCtx::default();
    let random = u32::from(tile.m3);
    ctx.random_bits = random;
    // Var 5F expone los random bits en el byte alto, igual que los demás
    // scopes de tesela (`GetRandomBits` + `GetVariable(0x5F)`).
    ctx.vars.insert(0x5F, random << 8);

    // `ObjectScopeResolver::GetVariable(0x40)`: yyxx repetido en las dos
    // mitades de la palabra. El formato local guarda dx/dy en MAP2 como lo
    // hace la ruta de construcción de objetos de este proyecto.
    let (dx, dy) = decode_object_tile_offset(tile.m2);
    let relative = u32::from(dy) << 20 | u32::from(dx) << 16 | u32::from(dy) << 8 | u32::from(dx);
    ctx.vars.insert(0x40, relative);

    // `GetTileSlope(tile) << 8 | GetTerrainType(tile)`. La marca MAP7 0x20
    // es la misma que usan las rutas road/rail para nieve/desierto importados.
    let terrain = if climate.uses_desert_patches() && tile.m7 & 0x20 != 0 {
        1
    } else if climate.uses_snow_ground() || tile.m7 & 0x20 != 0 {
        4
    } else {
        0
    };
    ctx.vars.insert(0x41, u32::from(tileh) << 8 | terrain);
    // MAP3HI es MAP4 en el save OpenTTD y contiene el frame de animación de
    // objetos cuando existe.
    ctx.vars.insert(0x43, u32::from(tile.m3hi));
    // `GetTileOwner(tile).base()`: los mapas del proyecto conservan el owner
    // directamente en m1 para MP_OBJECT.
    ctx.vars.insert(0x44, u32::from(tile.m1));
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameState;
    use crate::map::TileKind;
    use crate::sav;

    #[test]
    fn dual_fixture_resolves_transmitter_and_lighthouse_from_object_pool() {
        let raw = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/train_dual_pbs_curve_15_3.sav"
        ))
        .expect("fixture");
        let state = GameState::from_sav_game(sav::load(&raw).expect("load"));
        let tx = state
            .map
            .get(TileCoord::new(47, 33))
            .expect("transmitter tile");
        let lh = state
            .map
            .get(TileCoord::new(60, 55))
            .expect("lighthouse tile");
        assert_eq!(
            state.map.object_type_at(TileCoord::new(47, 33)),
            Some(u16::from(OBJECT_TYPE_TRANSMITTER))
        );
        assert_eq!(
            state.map.object_type_at(TileCoord::new(60, 55)),
            Some(u16::from(OBJECT_TYPE_LIGHTHOUSE))
        );
        assert!(object_id_from_tile(&tx).is_some());
        assert!(object_id_from_tile(&lh).is_some());
    }

    #[test]
    fn vanilla_object_ids_match_openttd_object_type_h() {
        assert_eq!(OBJECT_TYPE_TRANSMITTER, 0);
        assert_eq!(OBJECT_TYPE_LIGHTHOUSE, 1);
        assert_eq!(OBJECT_TYPE_STATUE_COMPANY, 2);
        assert_eq!(OBJECT_TYPE_OWNED_LAND, 3);
    }

    #[test]
    fn object_action2_context_matches_scope_tile_fields() {
        let tile = Tile {
            height: 0,
            kind: TileKind::Grass,
            mapt: MP_OBJECT_MAPT,
            m5: 5,
            m1: 7,
            m6: 0,
            m8: 0,
            m3: 0x2A,
            m2: object_tile_offset_byte(2, 3),
            m2_hi: 0,
            m7: 0x20,
            m3hi: 9,
        };
        let ctx = action2_eval_ctx_for_object_tile(tile, 5, Climate::SubTropical);
        assert_eq!(ctx.random_bits, 0x2A);
        assert_eq!(ctx.vars.get(&0x5F), Some(&0x2A00));
        assert_eq!(ctx.vars.get(&0x40), Some(&0x0032_0302));
        assert_eq!(ctx.vars.get(&0x41), Some(&0x0501));
        assert_eq!(ctx.vars.get(&0x43), Some(&9));
        assert_eq!(ctx.vars.get(&0x44), Some(&7));
    }
}
