//! Orden puro de los padres de sprites del viewport.
//!
//! OpenTTD no ordena los sprites del mapa únicamente por su coordenada de
//! pantalla: compara los prismas de mundo entregados a
//! `AddSortableSpriteToDraw`. Este módulo porta el algoritmo escalar de
//! `ViewportSortParentSprites` (`src/viewport.cpp`) sin depender de Bevy. La
//! integración deberá aplicar el vector de índices resultante a las entidades
//! de render y mantener juntos cada padre y sus children.

use std::cmp::Reverse;

/// Prisma de mundo inclusivo asociado a un padre sortable.
///
/// Los máximos pueden ser menores que los mínimos. OpenTTD admite extensiones
/// cero al crear las cajas y su sorter lo contempla mediante `max(min, max)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParentSpriteBounds {
    pub(crate) xmin: i32,
    pub(crate) ymin: i32,
    pub(crate) zmin: i32,
    pub(crate) xmax: i32,
    pub(crate) ymax: i32,
    pub(crate) zmax: i32,
}

impl ParentSpriteBounds {
    pub(crate) const fn new(
        xmin: i32,
        ymin: i32,
        zmin: i32,
        xmax: i32,
        ymax: i32,
        zmax: i32,
    ) -> Self {
        Self {
            xmin,
            ymin,
            zmin,
            xmax,
            ymax,
            zmax,
        }
    }

    fn min_sum(self) -> i64 {
        i64::from(self.xmin) + i64::from(self.ymin)
    }

    fn max_sum(self) -> i64 {
        i64::from(self.xmin.max(self.xmax)) + i64::from(self.ymin.max(self.ymax))
    }

    fn total_extent_sum(self) -> i64 {
        i64::from(self.xmin)
            + i64::from(self.xmax)
            + i64::from(self.ymin)
            + i64::from(self.ymax)
            + i64::from(self.zmin)
            + i64::from(self.zmax)
    }
}

/// El padre puede no dibujar imagen, pero conserva su caja y sus children.
///
/// Es el equivalente de `SPR_EMPTY_BOUNDING_BOX`: el padre participa en el
/// orden global aun cuando `ViewportDrawParentSprites` no emite su imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParentSpriteKind {
    Sprite { sprite_id: u32 },
    EmptyBoundingBox,
}

/// Child de pantalla que debe conservarse pegado a su padre sortable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChildScreenSprite {
    pub(crate) sprite_id: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) relative_to_parent_bounds: bool,
}

/// Entrada independiente de Bevy para el algoritmo de sorter de OpenTTD.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParentSprite {
    /// Identificador estable del productor; no participa en la comparación.
    pub(crate) id: u64,
    pub(crate) kind: ParentSpriteKind,
    pub(crate) bounds: ParentSpriteBounds,
    pub(crate) children: Vec<ChildScreenSprite>,
}

impl ParentSprite {
    pub(crate) fn sprite(id: u64, sprite_id: u32, bounds: ParentSpriteBounds) -> Self {
        Self {
            id,
            kind: ParentSpriteKind::Sprite { sprite_id },
            bounds,
            children: Vec::new(),
        }
    }

    pub(crate) fn empty_bounding_box(id: u64, bounds: ParentSpriteBounds) -> Self {
        Self {
            id,
            kind: ParentSpriteKind::EmptyBoundingBox,
            bounds,
            children: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SortState {
    Order(u64),
    Compared,
    Returned,
}

impl SortState {
    fn as_order(self) -> u64 {
        match self {
            Self::Order(order) => order,
            // Los sentinels C++ son UINT32_MAX y UINT32_MAX - 1. Sólo se
            // usan para conservar la prioridad al reencolar; u64 evita un
            // límite artificial del port puro.
            Self::Compared => u64::MAX,
            Self::Returned => u64::MAX - 1,
        }
    }
}

/// Devuelve los índices de `parents` en el orden que usa OpenTTD para dibujar.
///
/// La entrada no se muta: en C++ el campo temporal `order` vive dentro del
/// `ParentSpriteToDraw`; aquí se mantiene por separado para poder aplicar el
/// resultado después al renderer sin alterar los datos de spawn.
pub(crate) fn viewport_sort_parent_sprites(parents: &[ParentSprite]) -> Vec<usize> {
    if parents.len() < 2 {
        return (0..parents.len()).collect();
    }

    let mut active: Vec<usize> = (0..parents.len()).collect();
    // C++ ordena `pair<min_sum, ParentSpriteToDraw *>`: a suma igual la
    // dirección del vector de padres conserva el orden de inserción. El índice
    // reproduce ese segundo componente explícitamente.
    active.sort_by_key(|&index| (parents[index].bounds.min_sum(), index));

    // El C++ inicializa recorriendo `rbegin()` y apila cada elemento. El pop
    // inicial por tanto visita el orden de inserción (0, 1, ...).
    let mut stack: Vec<usize> = (0..parents.len()).rev().collect();
    let mut states: Vec<SortState> = (0..parents.len())
        .map(|index| SortState::Order((parents.len() - 1 - index) as u64))
        .collect();
    let mut next_order = parents.len() as u64;
    let mut output = Vec::with_capacity(parents.len());

    while let Some(current) = stack.pop() {
        match states[current] {
            SortState::Returned => continue,
            SortState::Compared => {
                output.push(current);
                states[current] = SortState::Returned;
                continue;
            }
            SortState::Order(_) => {}
        }

        let current_bounds = parents[current].bounds;
        let mut preceding = Vec::new();
        let scan_limit = current_bounds.max_sum();
        let mut position = 0;
        while position < active.len() && parents[active[position]].bounds.min_sum() <= scan_limit {
            let candidate = active[position];
            if candidate == current {
                active.remove(position);
                continue;
            }

            let candidate_bounds = parents[candidate].bounds;
            position += 1;

            if current_bounds.xmax < candidate_bounds.xmin
                || current_bounds.ymax < candidate_bounds.ymin
                || current_bounds.zmax < candidate_bounds.zmin
            {
                continue;
            }

            let overlaps_all_axes = current_bounds.xmin <= candidate_bounds.xmax
                && current_bounds.ymin <= candidate_bounds.ymax
                && current_bounds.zmin <= candidate_bounds.zmax;
            if overlaps_all_axes
                && current_bounds.total_extent_sum() <= candidate_bounds.total_extent_sum()
            {
                continue;
            }

            preceding.push(candidate);
        }

        if preceding.is_empty() {
            output.push(current);
            states[current] = SortState::Returned;
            continue;
        }

        if preceding.len() == 1 {
            let candidate = preceding[0];
            let candidate_bounds = parents[candidate].bounds;
            if candidate_bounds.xmax <= current_bounds.xmax
                && candidate_bounds.ymax <= current_bounds.ymax
                && candidate_bounds.zmax <= current_bounds.zmax
            {
                states[candidate] = SortState::Returned;
                states[current] = SortState::Returned;
                active.retain(|&index| index != candidate);
                output.push(candidate);
                output.push(current);
                continue;
            }
        }

        // `std::sort(..., a->order > b->order)`: se reencola primero el de
        // mayor orden para que el último push sea el primer pop.
        preceding.sort_by_key(|&index| Reverse(states[index].as_order()));
        states[current] = SortState::Compared;
        stack.push(current);
        for candidate in preceding {
            states[candidate] = SortState::Order(next_order);
            next_order += 1;
            stack.push(candidate);
        }
    }

    debug_assert_eq!(output.len(), parents.len());
    output
}

#[cfg(test)]
mod tests {
    use super::{
        ChildScreenSprite, ParentSprite, ParentSpriteBounds, ParentSpriteKind,
        viewport_sort_parent_sprites,
    };

    fn bounds(
        xmin: i32,
        ymin: i32,
        zmin: i32,
        xmax: i32,
        ymax: i32,
        zmax: i32,
    ) -> ParentSpriteBounds {
        ParentSpriteBounds::new(xmin, ymin, zmin, xmax, ymax, zmax)
    }

    fn sorted_ids(parents: &[ParentSprite]) -> Vec<u64> {
        viewport_sort_parent_sprites(parents)
            .into_iter()
            .map(|index| parents[index].id)
            .collect()
    }

    #[test]
    fn keeps_insertion_order_when_there_is_no_preceding_parent() {
        let parents = [
            ParentSprite::sprite(10, 100, bounds(0, 0, 0, 2, 2, 2)),
            ParentSprite::sprite(20, 200, bounds(0, 0, 0, 2, 2, 2)),
        ];

        // Caso de prismas coincidentes: el chequeo de suma de extents de
        // `ViewportSortParentSprites` evita introducir una dependencia falsa.
        assert_eq!(sorted_ids(&parents), vec![10, 20]);
    }

    #[test]
    fn moves_a_preceding_parent_before_a_later_world_prism() {
        let parents = [
            ParentSprite::sprite(20, 200, bounds(4, 4, 4, 6, 6, 6)),
            ParentSprite::sprite(10, 100, bounds(0, 0, 0, 2, 2, 2)),
        ];

        // Recorre el camino optimizado de un único precedente de C++.
        assert_eq!(sorted_ids(&parents), vec![10, 20]);
    }

    #[test]
    fn requeues_a_preceding_parent_when_the_fast_path_is_not_safe() {
        let parents = [
            ParentSprite::sprite(20, 200, bounds(0, 10, 0, 2, 12, 2)),
            ParentSprite::sprite(10, 100, bounds(0, 0, 0, 10, 2, 2)),
        ];

        // El padre 10 precede al 20, pero tiene xmax mayor: C++ no puede usar
        // el fast path y lo reencola antes de devolver el 20.
        assert_eq!(sorted_ids(&parents), vec![10, 20]);
    }

    #[test]
    fn empty_bounding_box_and_children_stay_attached_to_the_sorted_parent() {
        let mut empty = ParentSprite::empty_bounding_box(10, bounds(0, 0, 0, 2, 2, 2));
        empty.children.push(ChildScreenSprite {
            sprite_id: 777,
            x: -3,
            y: 4,
            relative_to_parent_bounds: true,
        });
        let parents = [
            ParentSprite::sprite(20, 200, bounds(4, 4, 4, 6, 6, 6)),
            empty,
        ];

        let order = viewport_sort_parent_sprites(&parents);
        assert_eq!(order, vec![1, 0]);
        assert_eq!(parents[order[0]].kind, ParentSpriteKind::EmptyBoundingBox);
        assert_eq!(parents[order[0]].children.len(), 1);
        assert_eq!(parents[order[0]].children[0].sprite_id, 777);
    }

    #[test]
    fn accepts_thin_empty_world_bounds_like_add_sortable_sprite_to_draw() {
        let parents = [
            // Un extent cero puede producir max < min; C++ usa max(min, max)
            // para continuar encontrando y removiendo el padre actual.
            ParentSprite::empty_bounding_box(20, bounds(10, 10, 10, 9, 9, 9)),
            ParentSprite::sprite(10, 100, bounds(0, 0, 0, 0, 0, 0)),
        ];

        assert_eq!(sorted_ids(&parents), vec![10, 20]);
    }

    #[test]
    fn matches_kale_road_stop_pair_from_post_sort_oracle() {
        // `Kale_TitleGame.sav`, región (225,2)..(226,2). `DrawTile_Station`
        // inserta `5982` y luego `5983` para cada parada. El stream C++ de
        // `ViewportSortParentSprites` deja primero el segundo padre: la caja
        // comienza 13 unidades antes y se solapa con la de `5982`.
        let parents = [
            ParentSprite::sprite(5982, 5982, bounds(3613, 32, 8, 3615, 47, 23)),
            ParentSprite::sprite(5983, 5983, bounds(3600, 32, 8, 3602, 47, 23)),
            ParentSprite::sprite(5982 + 10_000, 5982, bounds(3629, 32, 8, 3631, 47, 23)),
            ParentSprite::sprite(5983 + 10_000, 5983, bounds(3616, 32, 8, 3618, 47, 23)),
        ];

        assert_eq!(
            sorted_ids(&parents),
            vec![5983, 5982, 5983 + 10_000, 5982 + 10_000]
        );
    }
}
