//! Ciclo de teselas usado durante el arranque de un mundo nuevo.
//!
//! `GenerateWorld` no ejecuta un tick económico completo después de crear el
//! mapa. `OpenTTD` ejecuta 0x500 pasadas de `RunTileLoop` y sólo despacha el
//! callback de la tesela visitada (`landscape.cpp` / `genworld.cpp`). Mantener
//! esta ruta separada de [`GameState::step`](crate::GameState::step) evita que
//! el diagnóstico de mapas aleatorios adelante calendarios, vehículos o
//! producción antes de tiempo.

use crate::GameState;
use crate::cargodist::parity::Randomizer;
use crate::house_spec::{
    BUILDING_FLAG_IS_CHURCH, BUILDING_FLAG_IS_STADIUM, BUILDING_FLAG_SIZE_1X1,
    BUILDING_FLAG_SIZE_1X2, BUILDING_FLAG_SIZE_2X1, BUILDING_FLAG_SIZE_2X2, HouseSpec,
    get_town_radius_group,
};
use crate::map::{
    Map, Tile, TileCoord, TileKind, TileLoopState, collect_tile_loop_visits, coord_to_linear_index,
    industry_instance_id,
};

/// Cantidad de pasadas que `CreateRivers` ejecuta tras ensanchar ríos.
///
/// Es `TILE_UPDATE_FREQUENCY` en `landscape.cpp`; en ese intervalo cada
/// tesela recibe una visita LFSR y el agua estable queda marcada como
/// `non-flooding` en `MAP3` bit 0.
pub const LANDSCAPE_RIVER_TILE_LOOP_PASSES: u64 = 256;

/// Pasadas de `RunTileLoop` que `GenerateWorld` ejecuta después de crear el
/// mapa y las entidades iniciales (`genworld.cpp`).
pub const STARTUP_TILE_LOOP_PASSES: u64 = 0x500;

/// Ejecuta una pasada de `RunTileLoop` para la generación de un mundo nuevo.
///
/// `tick` es el contador de `TimerGameTick` usado por los callbacks de
/// paisaje. La función no avanza `GameState::tick`: durante la generación de
/// `OpenTTD` ese contador es independiente del calendario de la partida. El
/// estado LFSR persistente sí se actualiza en cada llamada.
///
/// El retorno es el número de teselas visitadas (útil para telemetría y
/// pruebas de la secuencia LFSR).
pub fn run_generation_tile_loop(state: &mut GameState, tick: u64) -> usize {
    run_generation_tile_loop_impl(state, tick, None)
}

/// Ejecuta la cola de tile loops de una partida nueva con el RNG global de
/// generación.
///
/// `OpenTTD` llama a `RunTileLoop`, incrementa `TimerGameTick::counter` y
/// repite la operación `0x500` veces antes de entregar el mundo al jugador.
/// El parámetro `passes` permite a los oráculos y tests aislar una prefija de
/// la cola; los consumidores de Nueva partida deben usar
/// [`STARTUP_TILE_LOOP_PASSES`]. El contador económico de [`GameState`] no se
/// modifica.
pub fn run_generation_tile_loops_with_rng(
    state: &mut GameState,
    rng: &mut Randomizer,
    passes: u64,
) -> usize {
    let mut visited = 0usize;
    for tick in 0..passes {
        visited =
            visited.saturating_add(run_generation_tile_loop_impl(state, tick, Some(&mut *rng)));
    }
    visited
}

/// Ejecuta la transición que `OpenTTD` hace al entregar un mundo nuevo al
/// `StateGameLoop`.
///
/// `GenerateWorld` termina con `TimerGameTick::counter == 0x500`. En el primer
/// tick regular se llama primero a `AnimateAnimatedTiles` con ese contador y,
/// después de incrementarlo, a `RunTileLoop` con `0x501`. La diferencia es
/// observable en `MAP6/MAP7` de industrias animadas y en los campos visitados
/// por el LFSR; omitirla deja al generador un tick detrás del mapa que exporta
/// el oráculo de `OpenTTD`.
pub fn run_first_regular_game_tick_with_rng(
    state: &mut GameState,
    rng: &mut Randomizer,
    startup_tick: u64,
) -> usize {
    let (width, height) = state.map.dimensions();
    let animated_industries: Vec<TileCoord> = (0..height)
        .flat_map(|y| (0..width).map(move |x| TileCoord::new(x.cast_signed(), y.cast_signed())))
        .filter(|&coord| {
            state
                .map
                .get(coord)
                .is_some_and(|tile| tile.kind == TileKind::Industry && tile.m6 & 0x03 != 0)
        })
        .collect();
    let _ = crate::map::industry_tile_anim::advance_startup_animated_industry_tiles(
        &mut state.map,
        startup_tick,
        &animated_industries,
        rng,
    );
    let visited = run_generation_tile_loop_impl(state, startup_tick.saturating_add(1), Some(rng));
    // `StateGameLoop` continúa con `CallLandscapeTick`; en una partida nueva
    // el primer callback con efecto observable suele ser `OnTick_Trees`, que
    // puede consumir el RNG global y convertir una tesela clear en árboles.
    // Reproducirlo aquí mantiene el raw exportado después del tick alineado
    // con OpenTTD, sin adelantar economía ni calendario.
    let _ = super::trees::advance_first_regular_tree_tick(
        &mut state.map,
        state.climate,
        startup_tick.saturating_add(1),
        rng,
    );
    visited
}

/// Variante interna de [`run_generation_tile_loop`] para la cola de
/// `CreateRivers`. Sólo esa frontera tiene que conservar el stream global de
/// `Random()` mientras los árboles de humedal ejecutan `TileLoop_Trees`.
fn run_generation_tile_loop_impl(
    state: &mut GameState,
    tick: u64,
    mut generation_rng: Option<&mut Randomizer>,
) -> usize {
    let visits = collect_tile_loop_visits(&state.map, tick, &mut state.cur_tileloop_tile);
    let visit_count = visits.len();

    // El tile loop de OpenTTD despacha cada tesela en el orden LFSR. Releer el
    // tile desde el mapa antes de cada callback conserva mutaciones producidas
    // por una tesela anterior de la misma pasada (en especial inundaciones).
    for (coord, snapshot) in &visits {
        let mut tile = state.map.get(*coord).unwrap_or(*snapshot);
        // `TileLoop_Clear` despacha la transición de clima antes del
        // crecimiento genérico. Releer la tesela conserva la mutación de
        // MAP3/MAP5 y decide si la segunda parte del callback debe ejecutarse.
        match state.climate {
            crate::world_gen::Climate::SubTropical => {
                crate::map::tree_tile_loop::tile_loop_clear_desert(
                    &mut state.map,
                    *coord,
                    state.climate,
                    state.world_seed,
                );
            }
            crate::world_gen::Climate::SubArctic => {
                crate::map::tree_tile_loop::tile_loop_clear_alps_at(
                    &mut state.map,
                    *coord,
                    state.snow_line_height,
                );
                if tile.kind == TileKind::Forest
                    && let Some(rng) = generation_rng.as_deref_mut()
                {
                    crate::map::tree_tile_loop::tile_loop_trees_alps_at(
                        &mut state.map,
                        *coord,
                        state.snow_line_height,
                        rng,
                    );
                }
            }
            _ => {}
        }
        tile = state.map.get(*coord).unwrap_or(tile);
        dispatch_generation_tile_loop_tile(state, tick, *coord, tile, &mut generation_rng);
    }

    // El último conjunto queda disponible para herramientas de diagnóstico,
    // igual que después de `phase_tile_loop` en la simulación normal.
    state.runtime.tile_loop_visited = visits;
    visit_count
}

#[allow(clippy::too_many_lines)] // Agrupa el orden observable de una visita de generación.
fn dispatch_generation_tile_loop_tile(
    state: &mut GameState,
    tick: u64,
    coord: TileCoord,
    tile: Tile,
    generation_rng: &mut Option<&mut Randomizer>,
) {
    match tile.kind {
        // TileLoop_Clear y TileLoop_Trees comparten el crecimiento de
        // hierba/campos y la actualización de árboles. El helper ya vuelve a
        // leer la tesela viva después de una transición desértica.
        TileKind::Forest
            if matches!(
                state.climate,
                crate::world_gen::Climate::Temperate
                    | crate::world_gen::Climate::SubArctic
                    | crate::world_gen::Climate::SubTropical
                    | crate::world_gen::Climate::Toyland
            ) && generation_rng.is_some() =>
        {
            // `TileLoop_Trees` delega primero una orilla a `TileLoop_Water`.
            // El callback puede cambiar la tesela, por lo que el procesador
            // de árbol la vuelve a leer del mapa.
            if (tile.m2 >> 6) & 0x07 == 3 {
                crate::map::water_flood::tile_loop_water_at(state, coord, tile);
            }
            if let Some(rng) = generation_rng.as_deref_mut() {
                crate::map::tree_tile_loop::process_generation_tree_growth_at(
                    &mut state.map,
                    state.climate,
                    tick,
                    rng,
                    coord,
                );
            }
        }
        TileKind::Forest | TileKind::CoalField | TileKind::Grass => {
            if tile.kind == TileKind::Grass
                && crate::map::tree_tile_loop::clear_ground_type(tile.m5)
                    == crate::world_gen::CLEAR_GROUND_FIELDS
            {
                tile_loop_clear_field(state, coord, tile);
            } else {
                crate::map::tree_tile_loop::process_tree_and_field_growth_from_visits(
                    &mut state.map,
                    tick,
                    state.world_seed,
                    &[(coord, tile)],
                );
            }
        }
        // TileLoop_Water es el único callback aplicable a MP_WATER y no debe
        // tratar los bordes MP_VOID como tiles válidos al inundar.
        TileKind::Water => crate::map::water_flood::tile_loop_water_at(state, coord, tile),
        // TileLoop_Industry primero deja que una industria sobre agua intente
        // inundar, luego ejecuta randomización, obra y animación. Las tres
        // operaciones consumen exactamente la visita actual y no barren de
        // nuevo el mapa.
        TileKind::Industry => {
            if crate::map::industry_terrain::industry_tile_on_water(tile) {
                crate::map::water_flood::tile_loop_water_at(state, coord, tile);
            }
            let Some(live_tile) = state.map.get(coord) else {
                return;
            };
            if live_tile.kind != TileKind::Industry {
                return;
            }
            let one = [(coord, live_tile)];
            let _ = crate::map::industry_random::
                advance_industry_tile_randomisation_from_visits_with_catalog_and_cargo_catalog(
                    &mut state.map,
                    tick,
                    state.world_seed,
                    &one,
                    &state.industries,
                    &state.towns,
                    &state.industry_tile_spec_catalog,
                    &state.industry_spec_catalog,
                    state.climate,
                    &state.cargo_spec_catalog,
                );
            // `MakeIndustryTileBigger` muta sólo la tesela visitada. La
            // simulación económica puede sincronizar un footprint, pero la
            // cola de generación debe conservar el desfase LFSR de cada parte.
            let was_completed = live_tile.m1 & 0x80 != 0;
            let construction_rollover =
                crate::map::industry_construction::industry_construction_counter(live_tile.m1) == 3;
            let _ = crate::map::industry_construction::advance_industry_construction_tile_loop_at(
                &mut state.map,
                coord,
            );
            // Aunque el grupo de animación vanilla no tenga callbacks, la
            // expresión C++ `TriggerIndustryTileAnimation_ConstructionStageChanged`
            // evalúa `Random()` antes de comprobar la máscara. Esa extracción
            // ocurre en cada cuarta visita (incluida la finalización).
            if construction_rollover && let Some(rng) = generation_rng.as_deref_mut() {
                let _ = rng.next();
            }
            // Completing a vanilla power-plant chimney creates a smoke effect
            // immediately. `ChimneySmokeInit` takes one word from the same
            // global RNG for its initial sprite/progress, even though the
            // effect is not serialized in the world-raw map.
            let completed_now = !was_completed
                && state
                    .map
                    .get(coord)
                    .is_some_and(|updated| updated.m1 & 0x80 != 0);
            if completed_now
                && state.map.get(coord).is_some_and(|updated| {
                    crate::map::industry_tile_anim::industry_gfx(&updated) == 8
                })
                && let Some(rng) = generation_rng.as_deref_mut()
            {
                let _chimney_smoke_seed = rng.next();
            }
            // OpenTTD retorna inmediatamente después de `MakeIndustryTileBigger`;
            // una tesela que termina su obra en esta visita no puede animarse
            // hasta una franja posterior.
            if !was_completed {
                return;
            }
            let one = [(coord, state.map.get(coord).unwrap_or(live_tile))];
            if let Some(rng) = generation_rng.as_deref_mut() {
                let _ = crate::map::industry_tile_anim::
                    advance_industry_tile_loop_events_from_visits_with_rng(
                        &mut state.map,
                        tick,
                        &one,
                        rng,
                    );
            } else {
                let _ =
                    crate::map::industry_tile_anim::advance_industry_tile_loop_events_from_visits(
                        &mut state.map,
                        tick,
                        &one,
                    );
            }
        }
        TileKind::Road => tile_loop_road(state, coord, tile),
        // `TileLoop_Town` avanza la construcción y consume el mismo stream
        // global cuando una casa terminada produce pasajeros/correo. La
        // economía que actualiza estaciones queda fuera de esta cola: sólo
        // se reproducen aquí los bytes de MAP3/MAP5 y las extracciones de
        // `Random()` que pueden desplazar los callbacks siguientes.
        TileKind::House => tile_loop_house(state, tick, coord, tile, generation_rng),
        // Vías, estaciones, objetos y depósitos tienen callbacks de tile loop
        // propios en OpenTTD. Para un mundo nuevo sus rutas de estado todavía
        // no tienen una mutación equivalente; dejar explícito el no-op evita
        // ejecutar lógica de economía del tick normal.
        TileKind::Rail
        | TileKind::RoadDepot
        | TileKind::RailDepot
        | TileKind::ShipDepot
        | TileKind::Airport
        | TileKind::RoadTunnel
        | TileKind::RailTunnel
        | TileKind::RoadBridge
        | TileKind::RailBridge
        | TileKind::Station
        | TileKind::Void
        | TileKind::Unknown(_) => {}
    }
}

/// Ejecuta la parte de `TileLoop_Town` observable durante la cola de arranque.
///
/// `OpenTTD` guarda el contador de construcción en los cinco bits bajos de
/// `MAP5`: los tres inferiores son el contador y los bits 3–4 la etapa. Al
/// terminar una obra escribe `MAP3` bit 7 y reinicia la edad (`MAP5 = 0`). Una
/// casa multitesela se identifica por las flags de la tesela norte y avanza
/// todas sus subteselas en una sola visita; las subteselas restantes tienen
/// flags vacías y no vuelven a avanzar el conjunto.
fn tile_loop_house(
    state: &mut GameState,
    tick: u64,
    coord: TileCoord,
    tile: Tile,
    generation_rng: &mut Option<&mut Randomizer>,
) {
    let house_id = tile.m8 & 0x0FFF;
    let Some(_house) =
        crate::house_spec::vanilla_or_newgrf_house(&state.house_spec_catalog, house_id)
    else {
        return;
    };
    // `NewHouseTileLoop` (CB21/CB22) may remove or replace a NewGRF house
    // before construction. Until that callback has a stateful generation
    // context, leaving the tile untouched is safer than consuming RNG or
    // marking it completed with vanilla semantics.
    if house_id >= crate::house_spec::NEW_HOUSE_OFFSET {
        return;
    }

    let Some(house) = HouseSpec::get(house_id) else {
        return;
    };

    if tile.m3 & 0x80 == 0 {
        // Las subteselas de una casa multitesela llevan un `HouseID` propio
        // pero su spec tiene flags vacías. OpenTTD sólo avanza el conjunto
        // cuando visita la tesela norte (la que conserva las flags de la
        // huella); avanzar una subtesela por separado terminaría la obra
        // varias visitas antes que el original.
        if house.building_flags == 0 {
            return;
        }
        for (dx, dy) in crate::house_spec::house_footprint_offsets(house.building_flags) {
            let part = TileCoord::new(coord.x + dx, coord.y + dy);
            advance_house_construction_tile(&mut state.map, part);
        }
        return;
    }

    let Some(rng) = generation_rng.as_deref_mut() else {
        return;
    };
    // `TileLoop_Town` tests the vanilla lift before taking the unconditional
    // random value used by cargo generation. `AddAnimatedTile` sólo cambia la
    // lista efímera, pero tanto la decisión como el orden de inserción deben
    // seguir el stream global para que `AnimateAnimatedTiles` tome luego el
    // mismo `RandomRange(7)`.
    let can_activate_lift = house.building_flags & crate::house_spec::BUILDING_FLAG_IS_ANIMATED
        != 0
        && !crate::map::house_lift::lift_has_destination(tile);
    if can_activate_lift {
        let activated = rng.chance16(1, 2);
        if activated {
            crate::map::add_house_lift_to_animation(&mut state.runtime.active_house_lifts, coord);
        }
    }

    // `r = Random()` siempre se extrae antes de la producción. El perfil de
    // Nueva partida de OpenTTD usa `TCGM_BITCOUNT` por defecto: la producción
    // sólo visita las dos especificaciones (pasajeros/correo) cuando los dos
    // bits altos del contador de tick coinciden con los dos bits bajos de
    // `TileIndex`. Esto también mantiene el stream exacto en la cola 0x500.
    let house_random = rng.next();
    let tile_index = coord_to_linear_index(coord, state.map.dimensions().0).unwrap_or(0);
    if ((tick >> 8) & 0x03) == u64::from(tile_index & 0x03) {
        let _passengers = rng.next();
        let _mail = rng.next();
    }

    // La renovación no es una pasada posterior de economía: `TileLoop_Town`
    // reutiliza `r = Random()` de esta misma visita después de generar carga.
    // Ejecutarla aquí conserva tanto el intercalado LFSR como el mapa que ven
    // las teselas siguientes de la franja.
    maybe_rebuild_vanilla_town_house(state, coord, tile, house, house_random, rng);
}

/// Máscara `BUILDING_HAS_1_TILE` de `OpenTTD`. Los bits no representan una
/// cuenta: `Size1x1` es el bit cero y los otros tamaños ocupan bits 2--4.
const VANILLA_HOUSE_FOOTPRINT_FLAGS: u8 = BUILDING_FLAG_SIZE_1X1
    | BUILDING_FLAG_SIZE_2X1
    | BUILDING_FLAG_SIZE_1X2
    | BUILDING_FLAG_SIZE_2X2;
const VANILLA_HOUSE_MULTI_TILE_FLAGS: u8 =
    BUILDING_FLAG_SIZE_2X1 | BUILDING_FLAG_SIZE_1X2 | BUILDING_FLAG_SIZE_2X2;

/// Replica la cola de reconstrucción de `TileLoop_Town` para una casa vanilla
/// completa. El contador se decrementa con wrapping, igual que el `uint16_t`
/// nativo: un valor cero no dispara una reconstrucción prematura.
fn maybe_rebuild_vanilla_town_house(
    state: &mut GameState,
    coord: TileCoord,
    tile: Tile,
    house: HouseSpec,
    house_random: u32,
    rng: &mut Randomizer,
) {
    if house.building_flags & VANILLA_HOUSE_FOOTPRINT_FLAGS == 0
        || tile.m3 & 0x20 != 0
        || tile.m5 < house.minimum_life
    {
        return;
    }

    // Validar antes de tocar el caché municipal. `ClearTownHouse` asume una
    // huella íntegra; un `.sav` corrupto no debe perder población sólo porque
    // una de sus subteselas quedó fuera del mapa.
    let footprint_is_intact = crate::house_spec::house_footprint_offsets(house.building_flags)
        .into_iter()
        .map(|(dx, dy)| TileCoord::new(coord.x + dx, coord.y + dy))
        .all(
            |part| matches!(state.map.get(part), Some(current) if current.kind == TileKind::House),
        );
    if !footprint_is_intact {
        return;
    }

    let persisted_town_id = u32::from(tile.m2) | (u32::from(tile.m2_hi) << 8);
    let Some(town_index) = state
        .towns
        .iter()
        .position(|town| town.id == persisted_town_id)
        .or_else(|| crate::town::nearest_town_index(&state.towns, coord).map(|(index, _)| index))
    else {
        return;
    };

    let rebuild_tile = {
        let town = &mut state.towns[town_index];
        if !town.is_growing {
            return;
        }

        town.time_until_rebuild = town.time_until_rebuild.wrapping_sub(1);
        if town.time_until_rebuild != 0 {
            return;
        }
        town.time_until_rebuild = u16::try_from((house_random >> 16) & 0xFF)
            .unwrap_or(0)
            .saturating_add(192);

        // `ClearTownHouse` reduce la población sólo si la construcción ya
        // había terminado, quita un edificio (no una subtesela) y actualiza
        // los flags únicos antes de que `TryBuildTownHouse` elija un reemplazo.
        town.population = town.population.saturating_sub(u32::from(house.population));
        town.num_houses = town.num_houses.saturating_sub(1);
        if house.building_flags & BUILDING_FLAG_IS_CHURCH != 0 {
            town.has_church = false;
        } else if house.building_flags & BUILDING_FLAG_IS_STADIUM != 0 {
            town.has_stadium = false;
        }
        crate::town::update_town_radius(town);

        rebuild_house_tile_closest_to_town(coord, house.building_flags, town.pos)
    };

    let Some(cleared) = clear_vanilla_town_house_footprint(&mut state.map, coord, house) else {
        // Un footprint corrupto no existe en OpenTTD; no hacemos que una
        // importación parcial borre una tesela aislada ni deje contadores
        // municipales inconsistentes.
        return;
    };
    state.runtime.landscape_tile_dirty.extend(cleared);

    // `GB(r, 24, 8) < 12` deja el solar vacío, sin otro sorteo.
    if (house_random >> 24) & 0xFF < 12 {
        return;
    }

    if let Some(base) = crate::world_gen::rebuild_vanilla_town_house_with_rng(
        &mut state.map,
        &mut state.towns[town_index],
        rebuild_tile,
        state.climate,
        state.snow_line_height,
        state.calendar.year,
        rng,
    ) {
        let flags = state
            .map
            .get(base)
            .and_then(|rebuilt| HouseSpec::get(rebuilt.m8 & 0x0FFF))
            .map_or(0, |rebuilt| rebuilt.building_flags);
        for (dx, dy) in crate::house_spec::house_footprint_offsets(flags) {
            state
                .runtime
                .landscape_tile_dirty
                .push(TileCoord::new(base.x + dx, base.y + dy));
        }
    }
}

/// Después de despejar un edificio multitile, `OpenTTD` desplaza el nuevo intento
/// hacia el centro de la ciudad para que las casas grandes no se alejen de la
/// red vial. `TileIndexToTileIndexDiffC(town, tile)` es `town - tile`.
fn rebuild_house_tile_closest_to_town(
    coord: TileCoord,
    building_flags: u8,
    town_pos: TileCoord,
) -> TileCoord {
    if building_flags & VANILLA_HOUSE_MULTI_TILE_FLAGS == 0 {
        return coord;
    }
    let x = (town_pos.x - coord.x).clamp(0, 1);
    let y = (town_pos.y - coord.y).clamp(0, 1);
    if building_flags & BUILDING_FLAG_SIZE_2X2 != 0 {
        TileCoord::new(coord.x + x, coord.y + y)
    } else if building_flags & BUILDING_FLAG_SIZE_1X2 != 0 {
        TileCoord::new(coord.x, coord.y + y)
    } else {
        TileCoord::new(coord.x + x, coord.y)
    }
}

/// `ClearTownHouse` + `DoClearSquare` para cada subtesela de una huella
/// vanilla. La comprobación previa mantiene la operación atómica ante mapas
/// corruptos; en un mapa válido coincide con el orden base, `+Y`, `+X`,
/// `+X+Y` de `OpenTTD`.
fn clear_vanilla_town_house_footprint(
    map: &mut Map,
    coord: TileCoord,
    house: HouseSpec,
) -> Option<Vec<TileCoord>> {
    let parts: Vec<_> = crate::house_spec::house_footprint_offsets(house.building_flags)
        .into_iter()
        .map(|(dx, dy)| TileCoord::new(coord.x + dx, coord.y + dy))
        .collect();
    if parts.is_empty()
        || parts
            .iter()
            .any(|&part| !matches!(map.get(part), Some(current) if current.kind == TileKind::House))
    {
        return None;
    }

    for &part in &parts {
        let mut clear = map.get(part)?;
        crate::map::clear_neighbour_non_flooding_states(map, part);
        clear.kind = TileKind::Grass;
        clear.mapt &= 0x0F;
        clear.m1 = crate::company::OWNER_NONE_M1;
        clear.m2 = 0;
        clear.m2_hi = 0;
        clear.m3 = 0;
        clear.m3hi = 0;
        clear.m5 = crate::world_gen::clear_ground_m5(crate::world_gen::CLEAR_GROUND_GRASS, 3);
        clear.m6 = 0;
        clear.m7 = 0;
        clear.m8 = 0;
        map.set_tile(part, clear).ok()?;
    }
    Some(parts)
}

/// Despacha una visita actual de `TileLoop_Town` con el stream global.
///
/// La cola de creación y el loop normal comparten el contrato de casas
/// vanilla: obra incompleta o callback `NewGRF` no consumen por este fallback;
/// una casa completa toma el `Random()` incondicional y, en la franja
/// `TCGM_BITCOUNT` correspondiente, los dos sorteos de pasajeros/correo.
/// Mantener la visita individual permite al caller conservar el intercalado
/// LFSR con `TileLoop_Industry` y futuros despachos de tesela.
pub(crate) fn advance_town_tile_loop_from_visit_with_rng(
    state: &mut GameState,
    tick: u64,
    coord: TileCoord,
    snapshot: Tile,
    rng: &mut Randomizer,
) {
    if snapshot.kind != TileKind::House {
        return;
    }
    let tile = state.map.get(coord).unwrap_or(snapshot);
    let mut shared_rng = Some(rng);
    tile_loop_house(state, tick, coord, tile, &mut shared_rng);
}

fn advance_house_construction_tile(map: &mut crate::map::Map, coord: TileCoord) {
    let Some(mut tile) = map.get(coord) else {
        return;
    };
    if tile.kind != TileKind::House || tile.m3 & 0x80 != 0 {
        return;
    }

    let next = (tile.m5 & 0x1F).wrapping_add(1) & 0x1F;
    tile.m5 = (tile.m5 & !0x1F) | next;
    if (tile.m5 >> 3) & 0x03 == 3 {
        tile.m3 |= 0x80;
        tile.m5 = 0;
    }
    let _ = map.set_tile(coord, tile);
}

/// Ejecuta la parte de `TileLoop_Road` que es observable durante la creación
/// de un mundo nuevo.
///
/// En ese momento todavía no hay vehículos ni obras viales, pero las calles
/// municipales sí pasan por el ajuste de decoración según la zona del pueblo.
/// `SetRoadside` sólo modifica `MAP6[3..=5]`; conservar los bits inferiores es
/// importante para los tipos de carretera/tranvía importados desde un save.
fn tile_loop_road(state: &mut GameState, coord: TileCoord, tile: Tile) {
    // Los depósitos tienen su propio callback en OpenTTD y nunca llegan aquí:
    // el decodificador los clasifica como `TileKind::RoadDepot`.
    if (tile.m5 >> 6) & 0x03 == 2 {
        return;
    }

    let Some(town) = state
        .towns
        .iter()
        .min_by_key(|town| (crate::economy::manhattan_distance(town.pos, coord), town.id))
    else {
        return;
    };

    // `_town_road_types` y `_town_road_types_2` de road_cmd.cpp. El primer
    // valor es el estado estable y el segundo el estado de transición que se
    // instala cuando la calle aún está en terreno desnudo.
    let zone = usize::from(get_town_radius_group(town, coord) as u8).min(4);
    let [desired, pre] = if state.climate == crate::world_gen::Climate::Toyland {
        // Toyland usa StreetLights en las zonas exteriores y no árboles.
        [[1_u8, 1], [2, 2], [3, 2], [3, 2], [3, 2]][zone]
    } else {
        [[1_u8, 1], [2, 2], [2, 2], [5, 5], [3, 2]][zone]
    };
    let current = (tile.m6 >> 3) & 0x07;
    let next = if current == desired {
        return;
    } else if current == pre {
        desired
    } else if current == 0 {
        pre
    } else {
        0
    };
    if next == current {
        return;
    }
    let mut updated = tile;
    updated.m6 = (updated.m6 & !0x38) | (next << 3);
    let _ = state.map.set_tile(coord, updated);
}

/// Reproduce la rama `CLEAR_FIELDS` de `TileLoop_Clear`.
///
/// Los campos son `MP_CLEAR` aunque se vean como una clase semántica de
/// terreno. Cada visita actualiza primero las cercas que limitan con una
/// tesela que no es campo y después avanza `MAP5`/`MAP3` con el contador de
/// ocho estados. Cuando un campo huérfano supera el tipo 7, `OpenTTD` lo
/// convierte en hierba de densidad 2; los campos ligados a una industria
/// vuelven al tipo 0 después del tipo 8.
/// Despacha una visita regular de `TileLoop_Clear` para un campo vivo.
///
/// La cola de generación ya utiliza [`tile_loop_clear_field`], pero la
/// simulación regular mantiene su propio despacho LFSR. Validar el nibble
/// crudo además del tipo semántico evita tratar como campos a una tesela que
/// el cargador represente como `Grass` por compatibilidad.
pub(crate) fn advance_clear_field_tile_loop_from_visit(
    state: &mut GameState,
    coord: TileCoord,
) -> bool {
    let Some(tile) = state.map.get(coord) else {
        return false;
    };
    if tile.kind != TileKind::Grass
        || tile.ottd_type_nibble() != 0
        || crate::map::tree_tile_loop::clear_ground_type(tile.m5)
            != crate::world_gen::CLEAR_GROUND_FIELDS
    {
        return false;
    }

    tile_loop_clear_field(state, coord, tile);
    state.map.get(coord).is_some_and(|updated| updated != tile)
}

fn tile_loop_clear_field(state: &mut GameState, coord: TileCoord, tile: Tile) {
    if tile.m3 & 0x10 != 0 {
        return;
    }

    let mut updated = tile;
    for direction in 0_u8..4 {
        if field_fence(updated, direction) != 0 {
            continue;
        }
        let (dx, dy) = crate::map::diag_dir_offset(direction);
        let neighbour = TileCoord::new(coord.x + dx, coord.y + dy);
        let neighbour_is_field = state.map.get(neighbour).is_some_and(|candidate| {
            candidate.kind == TileKind::Grass
                && candidate.ottd_type_nibble() == 0
                && crate::map::tree_tile_loop::clear_ground_type(candidate.m5)
                    == crate::world_gen::CLEAR_GROUND_FIELDS
        });
        if !neighbour_is_field {
            set_field_fence(&mut updated, direction, 3);
        }
    }

    let counter = crate::map::tree_tile_loop::clear_counter(updated.m5);
    if counter < 7 {
        updated.m5 = crate::map::tree_tile_loop::with_clear_counter(updated.m5, counter + 1);
    } else {
        updated.m5 = crate::map::tree_tile_loop::with_clear_counter(updated.m5, 0);
        let field_type = updated.m3 & 0x0F;
        if field_type >= 7
            && !state
                .industries
                .iter()
                .any(|industry| industry.instance_id == industry_instance_id(&updated))
        {
            // `MakeClear(tile, CLEAR_GRASS, 2)` resets every auxiliary map
            // plane except the low `TropicZone` nibble of `MAPT`.
            updated.kind = TileKind::Grass;
            updated.mapt &= 0x0F;
            updated.m1 = crate::company::OWNER_NONE_M1;
            updated.m2 = 0;
            updated.m2_hi = 0;
            updated.m3 = 0;
            updated.m3hi = 0;
            updated.m5 = crate::world_gen::clear_ground_m5(crate::world_gen::CLEAR_GROUND_GRASS, 2);
            updated.m6 = 0;
            updated.m7 = 0;
            updated.m8 = 0;
        } else {
            let next_type = if field_type < 8 { field_type + 1 } else { 0 };
            updated.m3 = (updated.m3 & !0x0F) | next_type;
        }
    }

    let _ = state.map.set_tile(coord, updated);
}

fn field_fence(tile: Tile, direction: u8) -> u8 {
    match direction & 3 {
        0 => (tile.m3 >> 5) & 0x07,   // DIAGDIR_NE
        1 => (tile.m3hi >> 2) & 0x07, // DIAGDIR_SE
        2 => (tile.m3hi >> 5) & 0x07, // DIAGDIR_SW
        _ => (tile.m6 >> 2) & 0x07,   // DIAGDIR_NW
    }
}

fn set_field_fence(tile: &mut Tile, direction: u8, value: u8) {
    let value = (value & 0x07) << 5;
    match direction & 3 {
        0 => tile.m3 = (tile.m3 & !0xE0) | value,
        1 => tile.m3hi = (tile.m3hi & !0x1C) | (value >> 3),
        2 => tile.m3hi = (tile.m3hi & !0xE0) | value,
        _ => tile.m6 = (tile.m6 & !0x1C) | (value >> 3),
    }
}

/// Reproduce las pasadas `RunTileLoop` finales de `CreateRivers`.
///
/// `GenerateLandscape` las corre antes de `GenerateClearTile`. El estado
/// temporal contiene sólo paisaje, por lo que no adelanta calendario,
/// vehículos, economía ni entidades de la partida; se conserva únicamente el
/// mapa y el cursor LFSR que afectan a los bytes de la frontera de generación.
pub fn run_landscape_river_tile_loops(
    map: &mut Map,
    climate: crate::world_gen::Climate,
    seed: u64,
) {
    let mut rng = Randomizer::new(seed as u32);
    run_landscape_river_tile_loops_with_rng(map, climate, seed, &mut rng);
}

/// Igual que [`run_landscape_river_tile_loops`], pero continúa el stream de
/// generación que ya usaron terreno y ríos.
///
/// Es `pub(crate)` para que [`crate::world_gen::apply_landscape_with_rng`]
/// no reinicie el RNG entre `CreateRivers` y `GenerateClearTile`.
pub(crate) fn run_landscape_river_tile_loops_with_rng(
    map: &mut Map,
    climate: crate::world_gen::Climate,
    seed: u64,
    rng: &mut Randomizer,
) {
    let _ = run_landscape_tile_loops_with_rng_and_cursor(
        map,
        climate,
        seed,
        rng,
        LANDSCAPE_RIVER_TILE_LOOP_PASSES,
        crate::world_gen::DEF_SNOW_LINE_HEIGHT,
        TileLoopState::default().cur_tileloop_tile,
    );
}

/// Ejecuta una cantidad explícita de pasadas del tile loop de generación y
/// devuelve el cursor LFSR para que otra frontera continúe la misma secuencia.
pub(crate) fn run_landscape_tile_loops_with_rng_and_cursor(
    map: &mut Map,
    climate: crate::world_gen::Climate,
    seed: u64,
    rng: &mut Randomizer,
    passes: u64,
    snow_line_height: u8,
    start_cursor: u32,
) -> u32 {
    // Mover en vez de clonar evita duplicar un mapa de hasta 4096² teselas
    // durante su creación. El placeholder no se observa: se reemplaza por el
    // mapa generado antes de devolver.
    let landscape = std::mem::replace(map, Map::new_flat(0, 0, 0));
    let mut state = GameState::from_map(landscape);
    state.climate = climate;
    state.world_seed = seed;
    state.snow_line_height = snow_line_height;
    state.cur_tileloop_tile = start_cursor;
    for _ in 0..passes {
        // `CreateRivers` no incrementa `TimerGameTick::counter` dentro de
        // este bucle. Usar siempre cero conserva tanto el callback manual de
        // tile 0 como cualquier regla dependiente del tick.
        run_generation_tile_loop_impl(&mut state, 0, Some(&mut *rng));
    }
    let cursor = state.cur_tileloop_tile;
    *map = state.map;
    cursor
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::map::{
        Map, TileCoord, TileKind, TileLoopState, WaterClass, collect_tile_loop_visits,
        set_water_class_m1,
    };

    #[test]
    fn generation_loop_uses_lfsr_without_advancing_calendar() {
        let state_map = Map::new_flat(64, 64, 1);
        let mut expected_cur = TileLoopState::default().cur_tileloop_tile;
        let expected = collect_tile_loop_visits(&state_map, 0, &mut expected_cur);
        let mut state = GameState::from_map(state_map);
        let before_tick = state.tick.get();
        let before_calendar = state.calendar;

        let visited = run_generation_tile_loop(&mut state, 0);

        assert_eq!(visited, expected.len());
        assert_eq!(state.cur_tileloop_tile, expected_cur);
        assert_eq!(state.runtime.tile_loop_visited.len(), expected.len());
        assert_eq!(state.tick.get(), before_tick);
        assert_eq!(state.calendar, before_calendar);
    }

    #[test]
    fn generation_loop_processes_clear_tiles_but_not_simulation_entities() {
        let mut map = Map::new_flat(64, 64, 1);
        let c = TileCoord::new(0, 0); // tile 0 se visita manualmente en tick 0
        map.set_kind(c, TileKind::Grass).unwrap();
        map.set_mapt_m5(c, 0, 0x00).unwrap();
        let mut state = GameState::from_map(map);
        let before_vehicles = state.vehicles.len();
        let before_industries = state.industries.len();

        run_generation_tile_loop(&mut state, 0);

        assert_eq!(state.vehicles.len(), before_vehicles);
        assert_eq!(state.industries.len(), before_industries);
        assert_ne!(state.map.get(c).unwrap().m5, 0x00);
    }

    #[test]
    fn startup_tile_loop_prefix_keeps_calendar_separate() {
        let map = Map::new_flat(64, 64, 1);
        let mut state = GameState::from_map(map);
        let mut expected_cursor = state.cur_tileloop_tile;
        let mut expected_visits = 0usize;
        for tick in 0..3 {
            expected_visits +=
                collect_tile_loop_visits(&state.map, tick, &mut expected_cursor).len();
        }
        let mut rng = Randomizer::new(42);
        let before_tick = state.tick.get();

        let visited = run_generation_tile_loops_with_rng(&mut state, &mut rng, 3);

        assert_eq!(visited, expected_visits);
        assert_eq!(state.tick.get(), before_tick);
        assert_eq!(state.cur_tileloop_tile, expected_cursor);
        assert_eq!(state.runtime.tile_loop_visited.len(), 16);
    }

    #[test]
    fn first_regular_game_tick_runs_animation_then_next_lfsr_pass() {
        let map = Map::new_flat(64, 64, 1);
        let mut state = GameState::from_map(map);
        let mut expected_cursor = state.cur_tileloop_tile;
        let _ = collect_tile_loop_visits(&state.map, 1281, &mut expected_cursor);
        let mut rng = Randomizer::new(42);

        let visited = run_first_regular_game_tick_with_rng(&mut state, &mut rng, 1280);

        assert_eq!(visited, 16);
        assert_eq!(state.cur_tileloop_tile, expected_cursor);
        assert_eq!(state.tick.get(), 0);
        assert_eq!(state.runtime.tile_loop_visited.len(), 16);
    }

    #[test]
    fn road_tile_loop_applies_town_roadside_transition_and_preserves_low_bits() {
        let mut map = Map::new_flat(16, 16, 0);
        let road = TileCoord::new(8, 8);
        let mut tile = map.get(road).unwrap();
        tile.kind = TileKind::Road;
        tile.mapt = crate::map::OTTD_MP_ROAD << 4;
        tile.m5 = 0x05;
        tile.m6 = 0x03;
        map.set_tile(road, tile).unwrap();

        let mut town = crate::town::Town {
            id: 0,
            pos: road,
            num_houses: 40,
            squared_town_zone_radius: [100, 64, 36, 16, 4],
            ..Default::default()
        };
        crate::town::update_town_radius(&mut town);
        let mut state = GameState::from_map(map);
        state.towns.push(town);

        // TownCentre uses Paved as the transition and StreetLights as the
        // stable value (`_town_road_types[4]`).
        let current = state.map.get(road).unwrap();
        tile_loop_road(&mut state, road, current);
        assert_eq!((state.map.get(road).unwrap().m6 >> 3) & 0x07, 2);
        assert_eq!(state.map.get(road).unwrap().m6 & 0x07, 3);
        let current = state.map.get(road).unwrap();
        tile_loop_road(&mut state, road, current);
        assert_eq!((state.map.get(road).unwrap().m6 >> 3) & 0x07, 3);
        assert_eq!(state.map.get(road).unwrap().m6 & 0x07, 3);
    }

    #[test]
    fn clear_field_tile_loop_updates_fences_and_reclaims_orphans() {
        let mut map = Map::new_flat(3, 3, 0);
        let field = TileCoord::new(1, 1);
        let mut tile = map.get(field).unwrap();
        tile.m5 = crate::world_gen::clear_ground_m5(crate::world_gen::CLEAR_GROUND_FIELDS, 3);
        tile.m2 = 0;
        map.set_tile(field, tile).unwrap();
        let mut state = GameState::from_map(map);

        let current = state.map.get(field).unwrap();
        tile_loop_clear_field(&mut state, field, current);
        let updated = state.map.get(field).unwrap();
        assert_eq!(crate::map::tree_tile_loop::clear_counter(updated.m5), 1);
        assert_eq!((updated.m3 >> 5) & 0x07, 3);
        assert_eq!((updated.m3hi >> 2) & 0x07, 3);
        assert_eq!((updated.m3hi >> 5) & 0x07, 3);
        assert_eq!((updated.m6 >> 2) & 0x07, 3);

        let mut orphan = updated;
        orphan.m3 = (orphan.m3 & !0x0F) | 7;
        orphan.m5 = crate::map::tree_tile_loop::with_clear_counter(orphan.m5, 7);
        state.map.set_tile(field, orphan).unwrap();
        let current = state.map.get(field).unwrap();
        tile_loop_clear_field(&mut state, field, current);
        let reclaimed = state.map.get(field).unwrap();
        assert_eq!(reclaimed.kind, TileKind::Grass);
        assert_eq!(reclaimed.m5, crate::world_gen::clear_ground_m5(0, 2));
        assert_eq!(reclaimed.m3, 0);
    }

    #[test]
    fn town_tile_loop_advances_multitile_construction_once_per_base_visit() {
        let mut map = Map::new_flat(4, 4, 0);
        let base = TileCoord::new(1, 1);
        let spec = crate::map::TownHouseSpec {
            house_id: 20, // vanilla 2×2 stadium footprint
            town_id: 0,
            random_bits: 0,
            construction_counter: 7,
            construction_stage: 2,
            is_protected: false,
            processing_time: 0,
        };
        let offsets = [(0, 0, 20), (0, 1, 21), (1, 0, 22), (1, 1, 23)];
        for (dx, dy, house_id) in offsets {
            let tile =
                crate::map::Tile::town_house(crate::map::TownHouseSpec { house_id, ..spec }, 0, 0);
            map.set_tile(TileCoord::new(base.x + dx, base.y + dy), tile)
                .unwrap();
        }
        let mut state = GameState::from_map(map);
        let mut rng = Randomizer::new(1);
        let mut generation_rng = Some(&mut rng);
        let current = state.map.get(base).unwrap();

        tile_loop_house(&mut state, 0, base, current, &mut generation_rng);

        for (dx, dy, _) in offsets {
            let tile = state
                .map
                .get(TileCoord::new(base.x + dx, base.y + dy))
                .unwrap();
            assert_ne!(tile.m3 & 0x80, 0);
            assert_eq!(tile.m5, 0);
        }
        // Construction returns before the completed-house RNG path.
        assert_eq!(rng, Randomizer::new(1));
    }

    #[test]
    fn town_tile_loop_consumes_house_random_and_default_cargo_draws() {
        let mut map = Map::new_flat(2, 2, 0);
        let coord = TileCoord::new(0, 0);
        map.set_tile(coord, crate::map::Tile::completed_house(0, 0, 0))
            .unwrap();
        let mut state = GameState::from_map(map);
        let mut actual = Randomizer::new(42);
        let mut expected = actual;
        let _ = expected.next();
        let _ = expected.next();
        let _ = expected.next();
        let mut generation_rng = Some(&mut actual);
        let current = state.map.get(coord).unwrap();

        tile_loop_house(&mut state, 0, coord, current, &mut generation_rng);

        assert_eq!(actual, expected);
    }

    #[test]
    fn town_rebuild_uses_the_house_random_without_an_extra_chance_roll() {
        let mut map = Map::new_flat(16, 16, 0);
        let coord = TileCoord::new(8, 8);
        let house = HouseSpec::get(6).expect("vanilla house");
        map.set_completed_house(coord, house.id, u8::MAX).unwrap();
        map.set_house_town_id(coord, 7).unwrap();
        let mut state = GameState::from_map(map);
        state.towns.push(crate::town::Town {
            id: 7,
            pos: coord,
            population: u32::from(house.population),
            num_houses: 1,
            is_growing: true,
            time_until_rebuild: 1,
            ..Default::default()
        });
        let mut rng = Randomizer::new(42);
        let before_rng = rng;
        let current = state.map.get(coord).expect("house tile");

        // En `TileLoop_Town`, el byte alto menor que 12 borra la casa y deja
        // el solar vacío. El byte 16--23 programa el próximo intento. No hay
        // otro `Chance16` ni `RandomRange` después de `r = Random()`.
        maybe_rebuild_vanilla_town_house(&mut state, coord, current, house, 0x0001_0000, &mut rng);

        let cleared = state.map.get(coord).expect("cleared tile");
        assert_eq!(cleared.kind, TileKind::Grass);
        assert_eq!(cleared.m1, crate::company::OWNER_NONE_M1);
        assert_eq!(
            cleared.m5,
            crate::world_gen::clear_ground_m5(crate::world_gen::CLEAR_GROUND_GRASS, 3)
        );
        assert_eq!(state.towns[0].time_until_rebuild, 193);
        assert_eq!(state.towns[0].population, 0);
        assert_eq!(state.towns[0].num_houses, 0);
        assert_eq!(rng, before_rng, "no debe sortear una segunda chance");
    }

    #[test]
    fn landscape_river_loops_mark_stable_water() {
        let mut map = Map::new_flat(64, 64, 0);
        for y in 0..64 {
            for x in 0..64 {
                let c = TileCoord::new(x, y);
                map.set_kind(c, TileKind::Water).unwrap();
                map.set_mapt_m5(c, 0x60, 0).unwrap();
                map.set_m1(c, set_water_class_m1(0x11, WaterClass::Sea))
                    .unwrap();
            }
        }

        run_landscape_river_tile_loops(&mut map, crate::world_gen::Climate::Temperate, 42);

        let stable = map
            .tiles()
            .iter()
            .filter(|tile| tile.kind == TileKind::Water && (tile.m3 & 1) != 0)
            .count();
        // Con el contador de tick fijo en cero, tile 0 ocupa una de las 16
        // visitas de cada pasada. OpenTTD por ello alcanza 1 + 15×256
        // posiciones LFSR, no un barrido completo de 4096 teselas.
        assert_eq!(stable, 1 + 15 * 256);
    }
}
