//! Modelo compartido de listas.

use std::cmp::Ordering;

/// Dirección de ordenación (clic repetido en el mismo chip la alterna).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SortDir {
    #[default]
    Asc,
    Desc,
}

impl SortDir {
    #[must_use]
    pub(crate) const fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    /// Aplica la dirección a un [`Ordering`] ya calculado (Asc = tal cual).
    #[must_use]
    pub(crate) const fn apply(self, ord: Ordering) -> Ordering {
        match self {
            Self::Asc => ord,
            Self::Desc => ord.reverse(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_flips() {
        assert_eq!(SortDir::Asc.toggle(), SortDir::Desc);
        assert_eq!(SortDir::Desc.toggle(), SortDir::Asc);
    }

    #[test]
    fn apply_reverses_on_desc() {
        assert_eq!(SortDir::Asc.apply(Ordering::Less), Ordering::Less);
        assert_eq!(SortDir::Desc.apply(Ordering::Less), Ordering::Greater);
    }
}
