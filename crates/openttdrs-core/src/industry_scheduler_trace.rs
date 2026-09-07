//! Muestras comparables del timer diario de industrias.
//!
//! El hook nativo de paridad escribe una fila inmediatamente después de
//! `_economy_industries_daily`. Estas estructuras conservan el mismo corte en
//! el candidato: no son estado de juego ni deben alterar el stream de RNG.

use crate::{
    GameState, INDUSTRY_BUILD_TYPE_COUNT, industry_daily_increment,
    news::{CALENDAR_BASE_YEAR, CALENDAR_DAYS_PER_YEAR},
};

/// El core conserva `date` relativo al año base, mientras `OpenTTD` exporta el
/// `Date` absoluto que lleva el chunk `DATE`. Esta adaptación sólo lleva el
/// campo numérico a la misma coordenada del contrato JSONL: año, mes, tick y
/// RNG se conservan sin normalizar para que el comparador exponga cualquier
/// diferencia real de importación.
fn openttd_trace_date(relative_date: u32) -> u32 {
    let base = u64::from(CALENDAR_BASE_YEAR).saturating_mul(CALENDAR_DAYS_PER_YEAR);
    relative_date.saturating_add(u32::try_from(base).unwrap_or(u32::MAX))
}

/// Reloj serializado por el contrato JSONL del scheduler industrial.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct IndustrySchedulerTraceClock {
    pub date: u32,
    pub year: u32,
    pub month: u8,
}

/// Estado del `Randomizer` global después de la muestra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct IndustrySchedulerTraceRandomState {
    pub state_0: u32,
    pub state_1: u32,
}

/// Fila de `ITBL` en el orden fijo de `IndustryType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct IndustrySchedulerTraceBuildData {
    #[serde(rename = "type")]
    pub industry_type: u16,
    pub probability: u32,
    pub min_number: u8,
    pub target_count: u16,
    pub max_wait: u16,
    pub wait_count: u16,
}

/// Industria visible al concluir el timer diario.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IndustrySchedulerTraceIndustry {
    pub id: u32,
    #[serde(rename = "type")]
    pub industry_type: u16,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub prod_level: u8,
    pub counter: u16,
    pub random: u16,
    pub selected_layout: u8,
    pub construction_type: u8,
}

/// Decisión ya tomada por una vuelta del scheduler diario.
///
/// Las opciones de fundación quedan vacías cuando el builder no pudo elegir
/// una especie, igual que el hook de `OpenTTD` que sólo recibe resultado tras
/// entrar a `PlaceIndustry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndustrySchedulerTraceAction {
    pub ordinal: u16,
    pub creation_percent: u8,
    pub tries_foundation: bool,
    pub industry_id: Option<u32>,
    pub foundation_type: Option<u16>,
    pub foundation_succeeded: Option<bool>,
}

impl IndustrySchedulerTraceAction {
    /// Acción de producción sobre una entidad existente del pool sparse.
    #[must_use]
    pub const fn production(ordinal: u16, creation_percent: u8, industry_id: Option<u32>) -> Self {
        Self {
            ordinal,
            creation_percent,
            tries_foundation: false,
            industry_id,
            foundation_type: None,
            foundation_succeeded: None,
        }
    }

    /// Acción que intentó (o no pudo intentar) fundar una industria.
    #[must_use]
    pub fn foundation(ordinal: u16, creation_percent: u8, result: Option<(u16, bool)>) -> Self {
        Self {
            ordinal,
            creation_percent,
            tries_foundation: true,
            industry_id: None,
            foundation_type: result.map(|(industry_type, _)| industry_type),
            foundation_succeeded: result.map(|(_, succeeded)| succeeded),
        }
    }
}

impl serde::Serialize for IndustrySchedulerTraceAction {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut row = serializer.serialize_struct("IndustrySchedulerTraceAction", 6)?;
        row.serialize_field("ordinal", &self.ordinal)?;
        row.serialize_field("creation_percent", &self.creation_percent)?;
        row.serialize_field(
            "branch",
            if self.tries_foundation {
                "foundation"
            } else {
                "production"
            },
        )?;
        row.serialize_field("industry", &self.industry_id)?;
        row.serialize_field("foundation_type", &self.foundation_type)?;
        row.serialize_field("foundation_succeeded", &self.foundation_succeeded)?;
        row.end()
    }
}

/// Estado completo que el contrato JSONL observa después de una jornada.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IndustrySchedulerTraceSample {
    pub tick: u64,
    pub calendar: IndustrySchedulerTraceClock,
    pub economy: IndustrySchedulerTraceClock,
    pub random_state: IndustrySchedulerTraceRandomState,
    pub industry_daily_change_counter: u32,
    pub industry_daily_increment: u32,
    pub wanted_inds: u32,
    pub change_loop: u16,
    pub builddata: Vec<IndustrySchedulerTraceBuildData>,
    pub industries: Vec<IndustrySchedulerTraceIndustry>,
    pub actions: Vec<IndustrySchedulerTraceAction>,
}

impl IndustrySchedulerTraceSample {
    /// Captura sólo datos ya existentes; nunca llama a `Random()`.
    #[must_use]
    pub fn from_state(
        state: &GameState,
        change_loop: u16,
        actions: Vec<IndustrySchedulerTraceAction>,
    ) -> Self {
        let (map_w, map_h) = state.map.dimensions();
        let builddata = (0..INDUSTRY_BUILD_TYPE_COUNT)
            .map(|index| {
                let data = state
                    .industry_builder
                    .builddata
                    .get(index)
                    .copied()
                    .unwrap_or_default();
                IndustrySchedulerTraceBuildData {
                    industry_type: u16::try_from(index).unwrap_or(u16::MAX),
                    probability: data.probability,
                    min_number: data.min_number,
                    target_count: data.target_count,
                    max_wait: data.max_wait,
                    wait_count: data.wait_count,
                }
            })
            .collect();

        let mut industries: Vec<_> = state
            .industries
            .iter()
            .map(|industry| IndustrySchedulerTraceIndustry {
                id: u32::from(industry.instance_id),
                industry_type: industry.spec.map_or_else(
                    || industry.newgrf_type_id.unwrap_or(u16::MAX),
                    |spec| spec.native_type().into(),
                ),
                x: Some(industry.pos.x),
                y: Some(industry.pos.y),
                prod_level: industry.prod_level,
                counter: industry.counter,
                random: industry.newgrf_random,
                selected_layout: industry.selected_layout,
                construction_type: industry.construction_type,
            })
            .collect();
        industries.sort_by_key(|industry| industry.id);

        Self {
            tick: state.tick.get(),
            calendar: IndustrySchedulerTraceClock {
                date: openttd_trace_date(state.calendar.date),
                year: state.calendar.year,
                month: state.calendar.month,
            },
            economy: IndustrySchedulerTraceClock {
                date: openttd_trace_date(state.economy_timer.date),
                year: state.economy_timer.year,
                month: state.economy_timer.month,
            },
            random_state: IndustrySchedulerTraceRandomState {
                state_0: state.random.state[0],
                state_1: state.random.state[1],
            },
            industry_daily_change_counter: state.global_economy.industry_daily_change_counter,
            industry_daily_increment: industry_daily_increment(map_w, map_h),
            wanted_inds: state.industry_builder.wanted_inds,
            change_loop,
            builddata,
            industries,
            actions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Industry, IndustrySpec, TileCoord};

    #[test]
    fn sample_is_ordered_and_does_not_consume_rng() {
        let mut state = GameState::new(64, 64);
        state.random.state = [7, 11];
        state.industries = vec![
            Industry::with_tiles_spec(
                TileCoord::new(4, 5),
                IndustrySpec::CoalMine.kind(),
                IndustrySpec::CoalMine,
                Vec::new(),
                0,
            )
            .with_instance_id(8),
            Industry::with_tiles_spec(
                TileCoord::new(1, 2),
                IndustrySpec::Factory.kind(),
                IndustrySpec::Factory,
                Vec::new(),
                0,
            )
            .with_instance_id(3),
        ];
        let before = state.random;

        let sample = IndustrySchedulerTraceSample::from_state(
            &state,
            0,
            vec![IndustrySchedulerTraceAction::production(0, 3, Some(3))],
        );

        assert_eq!(state.random, before);
        assert_eq!(sample.builddata.len(), INDUSTRY_BUILD_TYPE_COUNT);
        assert_eq!(sample.industries[0].id, 3);
        assert_eq!(sample.industries[1].id, 8);
        assert_eq!(sample.actions[0].industry_id, Some(3));
        assert_eq!(sample.calendar.date, 711_750);
    }
}
