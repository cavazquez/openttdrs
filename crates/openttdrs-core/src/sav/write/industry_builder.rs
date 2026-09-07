//! Serialización de `IndustryBuildData` (`IBLD` y `ITBL`).

use crate::game_state::GameState;
use crate::industry_builder::{
    INDUSTRY_BUILD_TYPE_COUNT, IndustryBuildData, IndustryTypeBuildData,
};

use super::super::SavError;
use super::chunks::table_chunk;

/// Registro único de `IBLD`.
pub(super) fn ibld_record(builder: &IndustryBuildData) -> Vec<u8> {
    builder.wanted_inds.to_be_bytes().to_vec()
}

/// Filas densas de `ITBL`, en orden de `IndustryType` nativo.
#[must_use]
pub(super) fn itbl_records(builder: &IndustryBuildData) -> Vec<Vec<u8>> {
    (0..INDUSTRY_BUILD_TYPE_COUNT)
        .map(|index| {
            let data = builder
                .builddata
                .get(index)
                .copied()
                .unwrap_or_else(IndustryTypeBuildData::new);
            itbl_record(data)
        })
        .collect()
}

fn itbl_record(data: IndustryTypeBuildData) -> Vec<u8> {
    let mut record = Vec::with_capacity(11);
    record.extend_from_slice(&data.probability.to_be_bytes());
    record.push(data.min_number);
    record.extend_from_slice(&data.target_count.to_be_bytes());
    record.extend_from_slice(&data.max_wait.to_be_bytes());
    record.extend_from_slice(&data.wait_count.to_be_bytes());
    record
}

/// Chunk `IBLD` canónico.
pub(super) fn ibld_chunk(state: &GameState) -> Result<Vec<u8>, SavError> {
    table_chunk(
        *b"IBLD",
        &[(6, "wanted_inds")],
        &[ibld_record(&state.industry_builder)],
    )
}

/// Chunk `ITBL` canónico con las 240 filas nativas.
pub(super) fn itbl_chunk(state: &GameState) -> Result<Vec<u8>, SavError> {
    table_chunk(
        *b"ITBL",
        &[
            (6, "probability"),
            (2, "min_number"),
            (4, "target_count"),
            (4, "max_wait"),
            (4, "wait_count"),
        ],
        &itbl_records(&state.industry_builder),
    )
}
