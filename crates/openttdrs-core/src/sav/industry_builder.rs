//! Estado del constructor automático de industrias (`IBLD` y `ITBL`).

use crate::industry_builder::{IndustryBuildData, IndustryTypeBuildData};

use super::chunks::{RawChunk, find_chunk};
use super::table::{SlValue, parse_table_chunk, record_get};

/// Decodifica el planificador persistente de industria de un save moderno.
///
/// Las filas ausentes de `ITBL` conservan el reset nativo (`max_wait = 1`),
/// igual que `ITBLChunkHandler::Load` antes de recorrer el pool sparse/denso.
#[must_use]
pub(crate) fn industry_builder_from_chunks(chunks: &[RawChunk]) -> IndustryBuildData {
    let mut builder = IndustryBuildData::new();
    if let Some(chunk) = find_chunk(chunks, "IBLD")
        && let Ok(rows) = parse_table_chunk(&chunk.body, false)
        && let Some((_, record)) = rows.first()
        && let Some(value) = record_get(record, "wanted_inds").and_then(SlValue::as_u64)
        && let Ok(value) = u32::try_from(value)
    {
        builder.wanted_inds = value;
    }

    let Some(chunk) = find_chunk(chunks, "ITBL") else {
        return builder;
    };
    let Ok(rows) = parse_table_chunk(&chunk.body, false) else {
        return builder;
    };
    for (index, record) in rows {
        let Ok(index) = usize::try_from(index) else {
            continue;
        };
        let Some(data) = builder.builddata.get_mut(index) else {
            // Los >239 son corrupción en OpenTTD. El importador de este
            // subconjunto conserva la parte representable sin ampliar el pool.
            continue;
        };
        read_type_build_data(data, &record);
    }
    builder
}

fn read_type_build_data(data: &mut IndustryTypeBuildData, record: &super::table::SlRecord) {
    if let Some(value) = record_get(record, "probability")
        .and_then(SlValue::as_u64)
        .and_then(|value| u32::try_from(value).ok())
    {
        data.probability = value;
    }
    if let Some(value) = record_get(record, "min_number")
        .and_then(SlValue::as_u64)
        .and_then(|value| u8::try_from(value).ok())
    {
        data.min_number = value;
    }
    if let Some(value) = record_get(record, "target_count")
        .and_then(SlValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
    {
        data.target_count = value;
    }
    if let Some(value) = record_get(record, "max_wait")
        .and_then(SlValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
    {
        data.max_wait = value;
    }
    if let Some(value) = record_get(record, "wait_count")
        .and_then(SlValue::as_u64)
        .and_then(|value| u16::try_from(value).ok())
    {
        data.wait_count = value;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::industry_builder::INDUSTRY_BUILD_TYPE_COUNT;
    use crate::sav::chunks::{CH_TABLE, RawChunk};
    use crate::sav::table::tests::build_table_body;

    #[test]
    fn reads_ibld_and_dense_itbl_rows_without_losing_native_reset_defaults() {
        let ibld = RawChunk {
            name: *b"IBLD",
            ch_type: CH_TABLE,
            body: build_table_body(&[(6, "wanted_inds")], &[123_456u32.to_be_bytes().to_vec()]),
        };
        let mut records = vec![vec![0; 11]; 6];
        let mut oil_rig = Vec::new();
        oil_rig.extend_from_slice(&6u32.to_be_bytes());
        oil_rig.push(1);
        oil_rig.extend_from_slice(&7u16.to_be_bytes());
        oil_rig.extend_from_slice(&9u16.to_be_bytes());
        oil_rig.extend_from_slice(&3u16.to_be_bytes());
        records[5] = oil_rig;
        let itbl = RawChunk {
            name: *b"ITBL",
            ch_type: CH_TABLE,
            body: build_table_body(
                &[
                    (6, "probability"),
                    (2, "min_number"),
                    (4, "target_count"),
                    (4, "max_wait"),
                    (4, "wait_count"),
                ],
                &records,
            ),
        };

        let builder = industry_builder_from_chunks(&[ibld, itbl]);
        assert_eq!(builder.wanted_inds, 123_456);
        assert_eq!(builder.builddata.len(), INDUSTRY_BUILD_TYPE_COUNT);
        assert_eq!(
            builder.builddata[5],
            IndustryTypeBuildData {
                probability: 6,
                min_number: 1,
                target_count: 7,
                max_wait: 9,
                wait_count: 3,
            }
        );
        assert_eq!(builder.builddata[6], IndustryTypeBuildData::new());
    }
}
