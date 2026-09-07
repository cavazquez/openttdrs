//! Planificador persistente de fundación de industrias (`IndustryBuildData`).
//!
//! `OpenTTD` mantiene este estado separado de las entidades `Industry`: `IBLD`
//! guarda la cantidad total deseada en punto fijo 16.16 e `ITBL` contiene una
//! fila por cada `IndustryType`.  Este módulo conserva esa representación y
//! las decisiones puras de `industry_cmd.cpp`; la colocación física y sus
//! consumos RNG se conectan en una etapa posterior.

use crate::cargodist::parity::Randomizer;
use crate::industry::IndustrySpec;
use crate::industry_spec::NUM_INDUSTRY_TYPES;
use crate::world_gen::{Climate, scale_by_size};

/// Cantidad de slots de `IndustryType` que serializa `OpenTTD` en `ITBL`.
pub const INDUSTRY_BUILD_TYPE_COUNT: usize = NUM_INDUSTRY_TYPES as usize;

/// Incremento base mensual de `IndustryBuildData::wanted_inds` para 256².
///
/// Es `0x38000 / (10 * 12)`: 3,5 industrias por década, en punto fijo 16.16.
pub const NEW_INDS_PER_MONTH: u32 = 0x38_000 / (10 * 12);

/// Días usados por `OpenTTD` para repartir los cambios diarios de industria.
pub const INDUSTRY_DAILY_CHANGE_DAYS: u32 = 31;

/// Fila persistida de `ITBL` (`IndustryTypeBuildData`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndustryTypeBuildData {
    /// Peso relativo de aparición durante una partida ya iniciada.
    pub probability: u32,
    /// Cantidad mínima que debe existir para ese tipo.
    pub min_number: u8,
    /// Objetivo actual asignado por `SetupTargetCount`.
    pub target_count: u16,
    /// Backoff máximo después de no encontrar ubicación.
    pub max_wait: u16,
    /// Días restantes de backoff.
    pub wait_count: u16,
}

impl IndustryTypeBuildData {
    /// Estado exacto de `IndustryTypeBuildData::Reset`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            probability: 0,
            min_number: 0,
            target_count: 0,
            max_wait: 1,
            wait_count: 0,
        }
    }

    /// Restablece la fila sin alterar su slot nativo.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for IndustryTypeBuildData {
    fn default() -> Self {
        Self::new()
    }
}

fn default_builddata() -> Vec<IndustryTypeBuildData> {
    vec![IndustryTypeBuildData::new(); INDUSTRY_BUILD_TYPE_COUNT]
}

/// Estado persistente del constructor automático de industrias (`IBLD` +
/// `ITBL`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndustryBuildData {
    /// Objetivo total 16.16 (`IBLD.wanted_inds`).
    #[serde(default)]
    pub wanted_inds: u32,
    /// Las 240 filas de `ITBL`, indexadas por `IndustryType` nativo.
    #[serde(default = "default_builddata")]
    pub builddata: Vec<IndustryTypeBuildData>,
}

impl Default for IndustryBuildData {
    fn default() -> Self {
        Self::new()
    }
}

impl IndustryBuildData {
    /// Constructor con las 240 filas en el estado nativo de reset.
    #[must_use]
    pub fn new() -> Self {
        Self {
            wanted_inds: 0,
            builddata: default_builddata(),
        }
    }

    /// Restaura el tamaño fijo de `ITBL` después de leer JSON legado o dañado.
    ///
    /// `OpenTTD` rechaza un `ITBL` nativo con más de 240 filas; el JSON propio
    /// no tiene ese framing, por eso se normaliza aquí antes de usarlo.
    pub fn ensure_type_count(&mut self) {
        self.builddata.truncate(INDUSTRY_BUILD_TYPE_COUNT);
        self.builddata
            .resize(INDUSTRY_BUILD_TYPE_COUNT, IndustryTypeBuildData::new());
    }

    /// `IndustryBuildData::Reset`: objetivo actual y filas limpias.
    pub fn reset(&mut self, current_industry_count: usize) {
        let current = u32::try_from(current_industry_count).unwrap_or(u32::MAX);
        self.wanted_inds = current.wrapping_shl(16);
        self.builddata = default_builddata();
    }

    /// Objetivo entero actual, descartando la fracción 16.16.
    #[must_use]
    pub const fn wanted_count(&self) -> u32 {
        self.wanted_inds >> 16
    }

    /// Devuelve una fila por su `IndustryType` nativo.
    #[must_use]
    pub fn type_data(&self, industry_type: u16) -> Option<&IndustryTypeBuildData> {
        self.builddata.get(usize::from(industry_type))
    }

    /// Devuelve una fila mutable por su `IndustryType` nativo.
    pub fn type_data_mut(&mut self, industry_type: u16) -> Option<&mut IndustryTypeBuildData> {
        self.ensure_type_count();
        self.builddata.get_mut(usize::from(industry_type))
    }

    /// `IndustryBuildData::EconomyMonthlyLoop`.
    ///
    /// `funded_only` corresponde a `ID_FUND_ONLY`: no aumenta el objetivo
    /// cuando la dificultad prohíbe la fundación automática.
    pub fn economy_monthly_loop(
        &mut self,
        current_industry_count: u32,
        map_w: u32,
        map_h: u32,
        funded_only: bool,
    ) {
        if funded_only {
            return;
        }
        let max_behind = 1_u32.saturating_add(scale_by_size(3, map_w, map_h).min(99));
        if current_industry_count.saturating_add(max_behind) >= self.wanted_count() {
            self.wanted_inds =
                self.wanted_inds
                    .wrapping_add(scale_by_size(NEW_INDS_PER_MONTH, map_w, map_h));
        }
    }

    /// Actualiza pesos y mínimos vanilla como `GetIndustryGamePlayProbability`.
    ///
    /// Los tipos `NewGRF` quedan en cero hasta que su callback de probabilidad y
    /// disponibilidad se conecten al runtime. Esta fase conserva sus filas al
    /// importar/exportar, pero no inventa una selección automática que el
    /// modelo aún no puede ejecutar.
    pub fn refresh_vanilla_gameplay_probabilities(
        &mut self,
        climate: Climate,
        calendar_year: u32,
        funded_only: bool,
    ) -> bool {
        self.ensure_type_count();
        // `GetIndustryTypeData` compara el resultado nuevo con la fila que ya
        // existe; no la resetea primero. Hacerlo en dos pasadas evita marcar
        // falsamente como cambiado cada tipo disponible y, por ende, consumir
        // `RandomRange` al volver a ejecutar `SetupTargetCount` sin motivo.
        let mut updated = vec![(0_u32, 0_u8); INDUSTRY_BUILD_TYPE_COUNT];
        if !funded_only {
            for &spec in IndustrySpec::specs_for_climate(climate) {
                updated[usize::from(spec.native_type())] = (
                    u32::from(spec.gameplay_probability(climate, calendar_year)),
                    0,
                );
            }
        }

        let mut changed = false;
        for (data, (probability, min_number)) in self.builddata.iter_mut().zip(updated) {
            changed |= data.probability != probability || data.min_number != min_number;
            data.probability = probability;
            // Ningún spec vanilla actual declara `CanCloseLastInstance`; el
            // campo queda persistible para NewGRF y futuras reglas.
            data.min_number = min_number;
        }
        changed
    }

    /// `SetupTargetCount` usando los pesos ya cargados en [`Self::builddata`].
    ///
    /// Esta variante detecta sólo cambios del objetivo entero. Quien actualice
    /// probabilidades debe usar [`Self::setup_vanilla_target_count`] o llamar
    /// a [`Self::setup_target_count_after_probability_change`] con la marca de
    /// cambio correspondiente para no omitir un reroll que `OpenTTD` sí haría.
    pub fn setup_target_count(&mut self, rng: &mut Randomizer) -> bool {
        self.setup_target_count_after_probability_change(false, rng)
    }

    /// Actualiza las probabilidades vanilla y ejecuta `SetupTargetCount`.
    pub fn setup_vanilla_target_count(
        &mut self,
        climate: Climate,
        calendar_year: u32,
        funded_only: bool,
        rng: &mut Randomizer,
    ) -> bool {
        let probability_changed =
            self.refresh_vanilla_gameplay_probabilities(climate, calendar_year, funded_only);
        self.setup_target_count_after_probability_change(probability_changed, rng)
    }

    /// Variante de `SetupTargetCount` que recibe el cambio detectado al
    /// refrescar probabilidades. Es útil para providers `NewGRF` en la etapa de
    /// runtime posterior.
    pub fn setup_target_count_after_probability_change(
        &mut self,
        probability_changed: bool,
        rng: &mut Randomizer,
    ) -> bool {
        self.ensure_type_count();
        let num_planned = self.builddata.iter().fold(0_u32, |total, data| {
            total.saturating_add(u32::from(data.target_count))
        });
        let changed = probability_changed || num_planned != self.wanted_count();
        if !changed {
            return false;
        }

        let mut force_build = 0_u32;
        let mut total_probability = 0_u32;
        for data in &mut self.builddata {
            force_build = force_build.saturating_add(u32::from(data.min_number));
            data.target_count = u16::from(data.min_number);
            total_probability = total_probability.wrapping_add(data.probability);
        }
        if total_probability == 0 {
            return true;
        }

        let mut total_amount = self.wanted_count().saturating_sub(force_build);
        while total_amount > 0 {
            let mut roll = rng.random_range(total_probability);
            for data in &mut self.builddata {
                if roll < data.probability {
                    data.target_count = data.target_count.saturating_add(1);
                    break;
                }
                roll = roll.saturating_sub(data.probability);
            }
            total_amount = total_amount.saturating_sub(1);
        }
        // Llegar hasta aquí siempre implicó un reroll, igual que el `while`
        // de `SetupTargetCount` nativo.
        true
    }

    /// Selecciona la especie que `TryBuildNewIndustry` intentará fundar.
    ///
    /// El caller debe haber ejecutado antes [`Self::setup_target_count`] (o
    /// su variante que refresca probabilidades) y debe completar la vuelta
    /// con [`Self::finish_automatic_build_attempt`], incluso si devuelve
    /// `None`: `OpenTTD` decrementa los backoffs en todos los caminos.
    #[must_use]
    pub fn select_automatic_build_type(
        &mut self,
        current_type_counts: &[u16],
        in_recession: bool,
        rng: &mut Randomizer,
    ) -> Option<u16> {
        self.ensure_type_count();

        let mut missing = 0_i32;
        let mut eligible = 0_u32;
        let mut total_probability = 0_u32;
        let mut forced: Option<(u16, i32)> = None;

        for (index, data) in self.builddata.iter().enumerate() {
            let current = current_type_counts.get(index).copied().unwrap_or(0);
            let difference = i32::from(data.target_count) - i32::from(current);
            missing = missing.saturating_add(difference);
            if data.wait_count > 0 || difference <= 0 {
                continue;
            }

            if current == 0
                && data.min_number > 0
                && forced.is_none_or(|(_, needed)| difference > needed)
            {
                forced = Some((u16::try_from(index).unwrap_or(u16::MAX), difference));
            }
            total_probability =
                total_probability.saturating_add(u32::try_from(difference).unwrap_or(0));
            eligible = eligible.saturating_add(1);
        }

        if in_recession || (forced.is_none() && (missing <= 0 || total_probability == 0)) {
            return None;
        }
        let (forced_type, _) = forced.unwrap_or((u16::MAX, 0));
        if forced_type != u16::MAX {
            return Some(forced_type);
        }
        if eligible == 0 {
            return None;
        }

        // `TryBuildNewIndustry` no sortea cuando sólo hay una especie elegible.
        let mut remaining = if eligible > 1 {
            rng.random_range(total_probability)
        } else {
            0
        };
        for (index, data) in self.builddata.iter().enumerate() {
            let current = current_type_counts.get(index).copied().unwrap_or(0);
            let difference = i32::from(data.target_count) - i32::from(current);
            if data.wait_count > 0 || difference <= 0 {
                continue;
            }
            if eligible == 1 || remaining < u32::try_from(difference).unwrap_or(0) {
                return u16::try_from(index).ok();
            }
            remaining = remaining.saturating_sub(u32::try_from(difference).unwrap_or(0));
        }

        // Las cuentas provienen de las mismas filas con las que se calculó
        // `eligible`, por lo que llegar aquí señalaría un modelo corrupto.
        None
    }

    /// Aplica el resultado de una vuelta de `TryBuildNewIndustry` y reduce
    /// todos los backoffs al final de la llamada, como el bucle nativo.
    pub fn finish_automatic_build_attempt(&mut self, industry_type: Option<u16>, succeeded: bool) {
        self.ensure_type_count();
        if let Some(industry_type) = industry_type
            && let Some(data) = self.type_data_mut(industry_type)
        {
            if succeeded {
                data.max_wait = (data.max_wait / 2).max(1);
            } else {
                data.wait_count = data.max_wait.saturating_add(1);
                data.max_wait = data.max_wait.saturating_add(2).min(1_000);
            }
        }
        for data in &mut self.builddata {
            data.wait_count = data.wait_count.saturating_sub(1);
        }
    }
}

/// Incremento diario derivado de la superficie del mapa (`StartupIndustryDailyChanges`).
#[must_use]
pub fn industry_daily_increment(map_w: u32, map_h: u32) -> u32 {
    let map_size = map_w.max(1).ilog2().saturating_add(map_h.max(1).ilog2());
    1_u32.checked_shl(map_size).unwrap_or(u32::MAX) / INDUSTRY_DAILY_CHANGE_DAYS
}

/// Acumula una jornada y devuelve cuántos cambios de industria corresponden.
///
/// Replica que `OpenTTD` suma el incremento, toma los bits 16..31 y luego
/// conserva sólo la fracción baja en `ECMY.industry_daily_change_counter`.
pub fn advance_industry_daily_change_counter(counter: &mut u32, map_w: u32, map_h: u32) -> u16 {
    *counter = counter.wrapping_add(industry_daily_increment(map_w, map_h));
    let change_loop = *counter >> 16;
    *counter &= 0xFFFF;
    u16::try_from(change_loop).unwrap_or(u16::MAX)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn reset_matches_native_240_slot_defaults() {
        let mut builder = IndustryBuildData::new();
        builder.wanted_inds = 123;
        builder.builddata[5] = IndustryTypeBuildData {
            probability: 7,
            min_number: 1,
            target_count: 9,
            max_wait: 12,
            wait_count: 3,
        };
        builder.reset(42);

        assert_eq!(builder.wanted_inds, 42 << 16);
        assert_eq!(builder.builddata.len(), INDUSTRY_BUILD_TYPE_COUNT);
        assert_eq!(builder.builddata[5], IndustryTypeBuildData::new());
    }

    #[test]
    fn oil_rig_probability_is_temperate_and_date_gated() {
        let mut builder = IndustryBuildData::new();
        assert!(builder.refresh_vanilla_gameplay_probabilities(Climate::Temperate, 1959, false));
        assert_eq!(builder.builddata[5].probability, 0);

        assert!(builder.refresh_vanilla_gameplay_probabilities(Climate::Temperate, 1960, false));
        assert_eq!(builder.builddata[5].probability, 6);

        assert!(builder.refresh_vanilla_gameplay_probabilities(Climate::SubArctic, 1960, false));
        assert_eq!(builder.builddata[5].probability, 0);
    }

    #[test]
    fn oil_wells_stop_after_1950() {
        let mut builder = IndustryBuildData::new();
        builder.refresh_vanilla_gameplay_probabilities(Climate::Temperate, 1950, false);
        assert_eq!(builder.builddata[11].probability, 5);
        builder.refresh_vanilla_gameplay_probabilities(Climate::Temperate, 1951, false);
        assert_eq!(builder.builddata[11].probability, 0);
    }

    #[test]
    fn monthly_wanted_growth_uses_native_map_scale_and_backlog_limit() {
        let mut builder = IndustryBuildData::new();
        builder.wanted_inds = 10 << 16;
        builder.economy_monthly_loop(8, 64, 64, false);
        assert_eq!(builder.wanted_inds, (10 << 16) + 120);

        builder.wanted_inds = 10 << 16;
        builder.economy_monthly_loop(6, 64, 64, false);
        assert_eq!(
            builder.wanted_inds,
            10 << 16,
            "dos industrias detrás aún avanza"
        );

        builder.wanted_inds = 10 << 16;
        builder.economy_monthly_loop(6, 256, 256, false);
        assert_eq!(builder.wanted_inds, (10 << 16) + NEW_INDS_PER_MONTH);

        builder.wanted_inds = 10 << 16;
        builder.economy_monthly_loop(6, 256, 256, true);
        assert_eq!(builder.wanted_inds, 10 << 16);
    }

    #[test]
    fn daily_counter_keeps_fractional_bits_at_native_map_scales() {
        assert_eq!(industry_daily_increment(64, 64), 132);
        assert_eq!(industry_daily_increment(256, 256), 2_114);
        assert_eq!(industry_daily_increment(512, 512), 8_456);

        let mut small = 0;
        for _ in 0..496 {
            assert_eq!(advance_industry_daily_change_counter(&mut small, 64, 64), 0);
        }
        assert_eq!(advance_industry_daily_change_counter(&mut small, 64, 64), 1);
        assert_eq!(small, 68);

        let mut normal = 0;
        for _ in 0..31 {
            assert_eq!(
                advance_industry_daily_change_counter(&mut normal, 256, 256),
                0
            );
        }
        assert_eq!(
            advance_industry_daily_change_counter(&mut normal, 256, 256),
            1
        );
        assert_eq!(normal, 2_112);

        let mut large = 0;
        for _ in 0..7 {
            assert_eq!(
                advance_industry_daily_change_counter(&mut large, 512, 512),
                0
            );
        }
        assert_eq!(
            advance_industry_daily_change_counter(&mut large, 512, 512),
            1
        );
        assert_eq!(large, 2_112);
    }

    #[test]
    fn target_count_uses_weighted_native_random_range() {
        let mut builder = IndustryBuildData::new();
        builder.wanted_inds = 2 << 16;
        builder.builddata[0].probability = 1;
        builder.builddata[5].probability = 2;
        let mut rng = Randomizer::new(1);

        assert!(builder.setup_target_count(&mut rng));
        assert_eq!(builder.builddata[0].target_count, 1);
        assert_eq!(builder.builddata[5].target_count, 1);
        assert_eq!(
            builder
                .builddata
                .iter()
                .map(|data| u32::from(data.target_count))
                .sum::<u32>(),
            2
        );
    }

    #[test]
    fn automatic_selection_forces_the_most_missing_minimum_without_rng() {
        let mut builder = IndustryBuildData::new();
        builder.builddata[2] = IndustryTypeBuildData {
            probability: 4,
            min_number: 1,
            target_count: 2,
            max_wait: 1,
            wait_count: 0,
        };
        builder.builddata[7] = IndustryTypeBuildData {
            probability: 4,
            min_number: 1,
            target_count: 3,
            max_wait: 1,
            wait_count: 0,
        };
        let counts = vec![0_u16; INDUSTRY_BUILD_TYPE_COUNT];
        let mut rng = Randomizer::new(7);
        let before = rng.state;

        assert_eq!(
            builder.select_automatic_build_type(&counts, false, &mut rng),
            Some(7),
            "la diferencia más alta de una especie obligatoria gana el empate"
        );
        assert_eq!(rng.state, before, "la ruta forced no llama a RandomRange");
    }

    #[test]
    fn automatic_selection_uses_weighted_roll_only_for_multiple_candidates() {
        let mut builder = IndustryBuildData::new();
        builder.builddata[2].target_count = 2;
        builder.builddata[7].target_count = 1;
        let counts = vec![0_u16; INDUSTRY_BUILD_TYPE_COUNT];
        let mut rng = Randomizer::new(1);

        // El primer Random() conocido de la semilla 1 escala a 0 en [0,3),
        // que pertenece al peso dos de la fila 2.
        assert_eq!(
            builder.select_automatic_build_type(&counts, false, &mut rng),
            Some(2)
        );
        assert_eq!(rng.state, [4_230_244_526, 536_870_911]);

        let mut one_candidate = IndustryBuildData::new();
        one_candidate.builddata[7].target_count = 1;
        let mut rng = Randomizer::new(1);
        let before = rng.state;
        assert_eq!(
            one_candidate.select_automatic_build_type(&counts, false, &mut rng),
            Some(7)
        );
        assert_eq!(rng.state, before, "una única fila elegible no consume RNG");
    }

    #[test]
    fn automatic_backoff_matches_failure_success_and_final_decrement() {
        let mut builder = IndustryBuildData::new();
        builder.builddata[5].max_wait = 1;

        builder.finish_automatic_build_attempt(Some(5), false);
        assert_eq!(builder.builddata[5].max_wait, 3);
        assert_eq!(
            builder.builddata[5].wait_count, 1,
            "max_wait + 1 se compensa con el decremento de la misma vuelta"
        );

        builder.finish_automatic_build_attempt(None, false);
        assert_eq!(builder.builddata[5].wait_count, 0);

        builder.finish_automatic_build_attempt(Some(5), true);
        assert_eq!(builder.builddata[5].max_wait, 1);
        assert_eq!(builder.builddata[5].wait_count, 0);
    }

    #[test]
    fn recession_skips_selection_without_consuming_rng() {
        let mut builder = IndustryBuildData::new();
        builder.builddata[5].target_count = 1;
        let counts = vec![0_u16; INDUSTRY_BUILD_TYPE_COUNT];
        let mut rng = Randomizer::new(9);
        let before = rng.state;

        assert_eq!(
            builder.select_automatic_build_type(&counts, true, &mut rng),
            None
        );
        assert_eq!(rng.state, before);
    }

    #[test]
    fn unchanged_runtime_probabilities_do_not_reroll_or_consume_rng() {
        let mut builder = IndustryBuildData::new();
        builder.wanted_inds = 4 << 16;
        let mut rng = Randomizer::new(1);

        assert!(builder.setup_vanilla_target_count(Climate::Temperate, 1960, false, &mut rng));
        let after_first_setup = rng.state;
        let targets_after_first_setup = builder.builddata.clone();

        assert!(!builder.setup_vanilla_target_count(Climate::Temperate, 1960, false, &mut rng));
        assert_eq!(rng.state, after_first_setup);
        assert_eq!(builder.builddata, targets_after_first_setup);

        assert!(builder.setup_vanilla_target_count(Climate::Temperate, 1959, false, &mut rng));
        assert_eq!(builder.builddata[5].probability, 0);
    }

    #[test]
    fn json_legacy_short_vector_is_normalized_before_use() {
        let mut builder = IndustryBuildData {
            wanted_inds: 0,
            builddata: vec![IndustryTypeBuildData::new()],
        };
        builder.ensure_type_count();
        assert_eq!(builder.builddata.len(), INDUSTRY_BUILD_TYPE_COUNT);
        assert_eq!(builder.builddata[239], IndustryTypeBuildData::new());
    }

    #[test]
    fn game_state_json_roundtrip_persists_the_builder() {
        let mut state = crate::GameState::new(64, 64);
        state.industry_builder.wanted_inds = (19 << 16) | 0x0011;
        *state
            .industry_builder
            .type_data_mut(5)
            .expect("Oil Rig slot") = IndustryTypeBuildData {
            probability: 6,
            min_number: 0,
            target_count: 4,
            max_wait: 11,
            wait_count: 2,
        };

        let json = state.save_json().expect("JSON");
        let loaded = crate::GameState::load_json(&json).expect("load JSON");
        assert_eq!(loaded.industry_builder, state.industry_builder);
    }
}
