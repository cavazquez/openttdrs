//! #371/#372: mutar filas SAV sin perder columnas no modeladas.

#![allow(clippy::expect_used)]

use super::*;
use crate::sav::table::{field_byte_ranges, parse_table_chunk, parse_table_layout, record_get};

fn rename_records(sparse: bool, raw: bool, names: [&str; 2], counters: [u8; 2]) -> Vec<Vec<u8>> {
    let mut records = Vec::new();
    for (ordinal, ((name, counter), index)) in
        names.into_iter().zip(counters).zip([3, 130]).enumerate()
    {
        if ordinal == 1 && !sparse {
            records.push(Vec::new());
        }
        let mut row = Vec::new();
        if sparse {
            codec::write_gamma(index, &mut row).expect("sparse index");
        }
        if raw {
            row.extend_from_slice(&[0xAB, 0xCD]);
            codec::write_str(name, &mut row).expect("name");
            row.extend_from_slice(&[counter, 0xCA, 0xFE]);
        } else {
            row.push(counter);
            codec::write_str(name, &mut row).expect("name");
            row.push(0); // Campo moderno omitido por el schema original.
        }
        records.push(row);
    }
    records
}

fn list_records(sparse: bool, raw: bool, values: [&[u8]; 2], counters: [u8; 2]) -> Vec<Vec<u8>> {
    let mut records = Vec::new();
    for (ordinal, ((values, counter), index)) in
        values.into_iter().zip(counters).zip([3, 130]).enumerate()
    {
        if ordinal == 1 && !sparse {
            records.push(Vec::new());
        }
        let mut row = Vec::new();
        if sparse {
            codec::write_gamma(index, &mut row).expect("sparse index");
        }
        if raw {
            row.extend_from_slice(&[0xAB, 0xCD]);
            codec::write_gamma(
                u32::try_from(values.len()).expect("list length fits gamma"),
                &mut row,
            )
            .expect("list length");
            row.extend_from_slice(values);
            row.extend_from_slice(&[counter, 0xCA, 0xFE]);
        } else {
            row.push(counter);
            codec::write_gamma(
                u32::try_from(values.len()).expect("list length fits gamma"),
                &mut row,
            )
            .expect("list length");
            row.extend_from_slice(values);
            row.push(0); // Campo moderno omitido por el schema original.
        }
        records.push(row);
    }
    records
}

fn dense_row_payloads(body: &[u8]) -> Vec<&[u8]> {
    let (_, mut offset, _) = parse_table_layout(body).expect("table header");
    let mut rows = Vec::new();
    loop {
        let length = crate::tnbp_decode::read_sl_gamma(body, &mut offset).expect("row length");
        if length == 0 {
            break;
        }
        let payload_len = usize::try_from(length - 1).expect("row length fits usize");
        let end = offset.checked_add(payload_len).expect("row end");
        assert!(end <= body.len(), "row payload within table");
        rows.push(&body[offset..end]);
        offset = end;
    }
    rows
}

#[test]
fn rename_preserves_unknown_columns_dense_holes_and_sparse_indices() {
    let long = "é".repeat(100);
    for sparse in [false, true] {
        for (before, after) in [("old", long.as_str()), (long.as_str(), "Ñ"), ("old", "")] {
            let raw_fields = [(4, "before"), (0x1A, "name"), (2, "known"), (4, "after")];
            let canonical_fields = [(2, "known"), (0x1A, "name"), (2, "newer")];
            let chunk_type = if sparse {
                super::super::chunks::CH_SPARSE_TABLE
            } else {
                super::super::chunks::CH_TABLE
            };
            let mut raw = chunks::table_chunk(
                *b"TEST",
                &raw_fields,
                &rename_records(sparse, true, [before, "untouched"], [7, 22]),
            )
            .expect("raw");
            raw[4] = chunk_type;
            let source = super::super::SavOpaqueChunk {
                name: *b"TEST",
                ch_type: chunk_type,
                body: raw[5..].to_vec(),
            };
            let mut canonical = chunks::table_chunk(
                *b"TEST",
                &canonical_fields,
                &rename_records(sparse, false, [after, "untouched"], [9, 99]),
            )
            .expect("canonical");
            canonical[4] = chunk_type;
            // El snapshot normalizó otra fila: su byte original 22 debe
            // conservarse aunque el modelo produzca 99 para ese campo.
            let snapshot = rename_records(sparse, false, [before, "untouched"], [7, 99]);
            let merged = chunks::table_chunk_with_passthrough_from_snapshot(
                Some(&source),
                canonical,
                Some(&snapshot),
            )
            .expect("merge");
            let mut expected = chunks::table_chunk(
                *b"TEST",
                &raw_fields,
                &rename_records(sparse, true, [after, "untouched"], [9, 22]),
            )
            .expect("expected");
            expected[4] = chunk_type;
            assert_eq!(merged, expected, "sparse={sparse}, {before:?} -> {after:?}");
            let rows = parse_table_chunk(&merged[5..], sparse).expect("valid framing");
            assert_eq!(
                rows.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                if sparse { vec![3, 130] } else { vec![0, 2] }
            );
        }
    }
}

#[test]
fn list_growth_preserves_unknown_columns_dense_holes_and_sparse_indices() {
    let before = vec![0x11; 127];
    let after = vec![0x22; 128];
    for sparse in [false, true] {
        for (old_values, new_values) in [
            (before.as_slice(), after.as_slice()),
            (after.as_slice(), &[0x33, 0x44][..]),
            (&[0x55][..], &[][..]),
        ] {
            let raw_fields = [(4, "before"), (0x12, "values"), (2, "known"), (4, "after")];
            let canonical_fields = [(2, "known"), (0x12, "values"), (2, "newer")];
            let chunk_type = if sparse {
                super::super::chunks::CH_SPARSE_TABLE
            } else {
                super::super::chunks::CH_TABLE
            };
            let mut raw = chunks::table_chunk(
                *b"TEST",
                &raw_fields,
                &list_records(sparse, true, [old_values, &[0x66, 0x77]], [7, 22]),
            )
            .expect("raw");
            raw[4] = chunk_type;
            let source = super::super::SavOpaqueChunk {
                name: *b"TEST",
                ch_type: chunk_type,
                body: raw[5..].to_vec(),
            };
            let mut canonical = chunks::table_chunk(
                *b"TEST",
                &canonical_fields,
                &list_records(sparse, false, [new_values, &[0x66, 0x77]], [9, 99]),
            )
            .expect("canonical");
            canonical[4] = chunk_type;
            let snapshot = list_records(sparse, false, [old_values, &[0x66, 0x77]], [7, 99]);
            let merged = chunks::table_chunk_with_passthrough_from_snapshot(
                Some(&source),
                canonical,
                Some(&snapshot),
            )
            .expect("merge");
            let mut expected = chunks::table_chunk(
                *b"TEST",
                &raw_fields,
                &list_records(sparse, true, [new_values, &[0x66, 0x77]], [9, 22]),
            )
            .expect("expected");
            expected[4] = chunk_type;
            assert_eq!(
                merged,
                expected,
                "sparse={sparse}, {} -> {}",
                old_values.len(),
                new_values.len()
            );
            let rows = parse_table_chunk(&merged[5..], sparse).expect("valid framing");
            assert_eq!(
                rows.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                if sparse { vec![3, 130] } else { vec![0, 2] }
            );
        }
    }
}

#[test]
fn rename_preserves_large_unknown_column_across_three_byte_row_length() {
    let mut row = Vec::new();
    codec::write_str("old", &mut row).expect("old name");
    codec::write_gamma(16_370, &mut row).expect("list length");
    row.extend(std::iter::repeat_n(0xA5, 16_370));
    let raw = chunks::table_chunk(*b"TEST", &[(0x1A, "name"), (0x12, "future")], &[row])
        .expect("raw fits two-byte row length");
    let source = super::super::SavOpaqueChunk {
        name: *b"TEST",
        ch_type: super::super::chunks::CH_TABLE,
        body: raw[5..].to_vec(),
    };
    let name = "Transportes del Sur y del Litoral";
    let mut row = Vec::new();
    codec::write_str(name, &mut row).expect("new name");
    let canonical = chunks::table_chunk(*b"TEST", &[(0x1A, "name")], &[row]).expect("canonical");
    let merged = chunks::table_chunk_with_passthrough_from_snapshot(Some(&source), canonical, None)
        .expect("merge");
    let (_, header_end, fields) = parse_table_layout(&source.body).expect("header");
    let body = &merged[5..];
    assert_eq!(&body[..header_end], &source.body[..header_end]);
    let mut offset = header_end;
    let length = crate::tnbp_decode::read_sl_gamma(body, &mut offset).expect("new row length");
    assert!(length >= 16_384);
    assert_eq!(offset - header_end, 3);
    let row = &body[offset..offset + length as usize - 1];
    let ranges = field_byte_ranges(&fields, row).expect("new ranges");
    let (_, start, end) = ranges
        .iter()
        .find(|(name, _, _)| name == "future")
        .expect("future");
    let mut old_offset = header_end;
    let old_length =
        crate::tnbp_decode::read_sl_gamma(&source.body, &mut old_offset).expect("old row length");
    assert_eq!(old_offset - header_end, 2);
    let old_row = &source.body[old_offset..old_offset + old_length as usize - 1];
    assert_eq!(&row[*start..*end], &old_row[4..]);
    let rows = parse_table_chunk(body, false).expect("valid table");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        record_get(&rows[0].1, "name").and_then(|v| v.as_str()),
        Some(name)
    );
}

#[test]
fn rename_falls_back_when_row_identity_or_nested_struct_size_changes() {
    let mut raw_header = Vec::new();
    raw_header.push(0x1B);
    codec::write_str("entries", &mut raw_header).expect("struct field");
    raw_header.push(2);
    codec::write_str("future", &mut raw_header).expect("future field");
    raw_header.push(0);
    raw_header.push(2);
    codec::write_str("value", &mut raw_header).expect("nested field");
    raw_header.push(0);
    let mut raw_record = Vec::new();
    codec::write_gamma(1, &mut raw_record).expect("struct count");
    raw_record.extend_from_slice(&[7, 0xAA]);
    let raw = chunks::raw_table_chunk(
        *b"TEST",
        &raw_header,
        &[raw_record],
        super::super::chunks::CH_TABLE,
    )
    .expect("raw");
    let source = super::super::SavOpaqueChunk {
        name: *b"TEST",
        ch_type: super::super::chunks::CH_TABLE,
        body: raw[5..].to_vec(),
    };
    let mut canonical_header = Vec::new();
    canonical_header.push(0x1B);
    codec::write_str("entries", &mut canonical_header).expect("struct field");
    canonical_header.push(0);
    canonical_header.push(2);
    codec::write_str("value", &mut canonical_header).expect("nested field");
    canonical_header.push(0);
    let mut canonical_record = Vec::new();
    codec::write_gamma(2, &mut canonical_record).expect("struct count");
    canonical_record.extend_from_slice(&[7, 8]);
    let canonical = chunks::raw_table_chunk(
        *b"TEST",
        &canonical_header,
        &[canonical_record],
        super::super::chunks::CH_TABLE,
    )
    .expect("canonical");
    let merged =
        chunks::table_chunk_with_passthrough_from_snapshot(Some(&source), canonical.clone(), None)
            .expect("fallback");
    assert_eq!(merged, canonical);

    let canonical = chunks::raw_table_chunk(
        *b"TEST",
        &canonical_header,
        &[vec![1, 7], vec![1, 8]],
        super::super::chunks::CH_TABLE,
    )
    .expect("topology changed");
    let merged =
        chunks::table_chunk_with_passthrough_from_snapshot(Some(&source), canonical.clone(), None)
            .expect("fallback");
    assert_eq!(merged, canonical);

    let mut source = source;
    source.ch_type = super::super::chunks::CH_SPARSE_TABLE;
    let mut raw =
        chunks::table_chunk(*b"TEST", &[(0x1A, "name")], &[vec![3, 1, b'a']]).expect("sparse raw");
    raw[4] = source.ch_type;
    source.body = raw[5..].to_vec();
    let mut canonical = chunks::table_chunk(*b"TEST", &[(0x1A, "name")], &[vec![4, 1, b'b']])
        .expect("sparse canonical");
    canonical[4] = source.ch_type;
    let merged =
        chunks::table_chunk_with_passthrough_from_snapshot(Some(&source), canonical.clone(), None)
            .expect("changed sparse id");
    assert_eq!(merged, canonical);
}

#[test]
fn rename_falls_back_when_name_descriptor_changes() {
    let raw = chunks::table_chunk(
        *b"TEST",
        &[(0x1A, "name"), (2, "future")],
        &[vec![1, b'a', 0xAA]],
    )
    .expect("raw");
    let source = super::super::SavOpaqueChunk {
        name: *b"TEST",
        ch_type: super::super::chunks::CH_TABLE,
        body: raw[5..].to_vec(),
    };
    let canonical = chunks::table_chunk(*b"TEST", &[(0x0A, "name")], &[vec![2, b'b', b'c']])
        .expect("different descriptor");
    let snapshot = [vec![1, b'a']];
    for snapshot in [None, Some(snapshot.as_slice())] {
        let merged = chunks::table_chunk_with_passthrough_from_snapshot(
            Some(&source),
            canonical.clone(),
            snapshot,
        )
        .expect("fallback");
        assert_eq!(merged, canonical);
    }
}

#[test]
fn native_town_psa_list_growth_preserves_other_city_fields() {
    let original = include_bytes!("../../../tests/fixtures/train_pbs_15_3.sav");
    let (original_payload, _) = super::super::container::decompress(original).expect("container");
    let original_chunks = super::super::chunks::parse_chunks(&original_payload).expect("chunks");
    let original_city =
        super::super::chunks::find_chunk(&original_chunks, "CITY").expect("native CITY");
    assert_eq!(original_city.ch_type, super::super::chunks::CH_TABLE);
    let (_, header_end, fields) = parse_table_layout(&original_city.body).expect("header");
    assert!(fields.iter().any(|field| field.name == "psa_list"));
    assert!(fields.iter().any(|field| field.name == "name"));

    let mut state = GameState::from_sav_game(super::super::load(original).expect("native SAV"));
    let town = state.towns.first_mut().expect("native town");
    let town_id = town.id;
    let old_refs = state
        .sav_town_persistent_storage_ids
        .get(&town_id)
        .cloned()
        .unwrap_or_default();
    let grfid = 0xD1CE_BA5Eu32;
    town.newgrf_persistent_regs.insert(
        grfid,
        std::collections::HashMap::from([(7, 0xCAFE_BABEu32)]),
    );
    let output = save_to_bytes_with(&state, SavContainer::Ottn).expect("SAV with town PSA");
    let (payload, _) = super::super::container::decompress(&output).expect("output container");
    let output_chunks = super::super::chunks::parse_chunks(&payload).expect("output chunks");
    let output_city =
        super::super::chunks::find_chunk(&output_chunks, "CITY").expect("output CITY");
    assert_eq!(output_city.ch_type, original_city.ch_type);
    assert_eq!(
        &output_city.body[..header_end],
        &original_city.body[..header_end]
    );
    let old_rows = dense_row_payloads(&original_city.body);
    let new_rows = dense_row_payloads(&output_city.body);
    assert_eq!(old_rows.len(), new_rows.len());
    for (old_row, new_row) in old_rows.iter().zip(&new_rows) {
        let old_ranges = field_byte_ranges(&fields, old_row).expect("native ranges");
        let new_ranges = field_byte_ranges(&fields, new_row).expect("output ranges");
        for ((field, start, end), (_, new_start, new_end)) in old_ranges.iter().zip(&new_ranges) {
            if field != "psa_list" {
                assert_eq!(
                    &old_row[*start..*end],
                    &new_row[*new_start..*new_end],
                    "{field}"
                );
            }
        }
    }
    let reloaded = super::super::load(&output).expect("reimport");
    let refs = reloaded
        .town_persistent_storage_ids
        .get(&town_id)
        .expect("new town PSA reference");
    assert_eq!(refs.len(), old_refs.len() + 1);
    let storage_id = reloaded
        .persistent_storages
        .iter()
        .find(|storage| storage.grfid == grfid)
        .map(|storage| storage.storage_id)
        .expect("new PSAC row");
    assert!(refs.contains(&storage_id));
    if let Ok(path) = std::env::var("OPENTTDRS_DUMP_TOWN_PSA_NATIVE_SAV") {
        std::fs::write(path, output).expect("dump for OpenTTD");
    }
}

#[test]
fn native_company_rename_preserves_other_plyr_fields() {
    let original = include_bytes!("../../../tests/fixtures/train_pbs_15_3.sav");
    let (original_payload, _) = super::super::container::decompress(original).expect("container");
    let original_chunks = super::super::chunks::parse_chunks(&original_payload).expect("chunks");
    let original_plyr =
        super::super::chunks::find_chunk(&original_chunks, "PLYR").expect("native PLYR");
    let (_, header_end, fields) = parse_table_layout(&original_plyr.body).expect("header");
    assert!(fields.iter().any(|field| field.name == "name_1"));

    for name in ["Transportes del Sur y del Litoral", "Ñ", ""] {
        let mut state = GameState::from_sav_game(super::super::load(original).expect("native SAV"));
        state.companies[0].name = name.to_owned();
        let output = save_to_bytes_with(&state, SavContainer::Ottn).expect("renamed SAV");
        let (payload, _) = super::super::container::decompress(&output).expect("output container");
        let output_chunks = super::super::chunks::parse_chunks(&payload).expect("output chunks");
        let output_plyr =
            super::super::chunks::find_chunk(&output_chunks, "PLYR").expect("output PLYR");
        assert_eq!(output_plyr.ch_type, original_plyr.ch_type);
        assert_eq!(
            &output_plyr.body[..header_end],
            &original_plyr.body[..header_end]
        );

        // El fixture nativo tiene una compañía. Comparamos los bytes de cada
        // columna, incluidas las desconocidas y los structs normalizados al
        // importar, sin confiar en que el modelo semántico los represente.
        let mut old_offset = header_end;
        let mut new_offset = header_end;
        let old_len = crate::tnbp_decode::read_sl_gamma(&original_plyr.body, &mut old_offset)
            .expect("native length") as usize
            - 1;
        let new_len = crate::tnbp_decode::read_sl_gamma(&output_plyr.body, &mut new_offset)
            .expect("output length") as usize
            - 1;
        let old_row = &original_plyr.body[old_offset..old_offset + old_len];
        let new_row = &output_plyr.body[new_offset..new_offset + new_len];
        let old_ranges = field_byte_ranges(&fields, old_row).expect("native ranges");
        let new_ranges = field_byte_ranges(&fields, new_row).expect("output ranges");
        for ((field, start, end), (_, new_start, new_end)) in old_ranges.iter().zip(&new_ranges) {
            if field != "name" {
                assert_eq!(
                    &old_row[*start..*end],
                    &new_row[*new_start..*new_end],
                    "{field}"
                );
            }
        }
        let rows = parse_table_chunk(&output_plyr.body, false).expect("output rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            record_get(&rows[0].1, "name").and_then(|v| v.as_str()),
            Some(name)
        );
        let reloaded = super::super::load(&output).expect("reimport");
        if !name.is_empty() {
            assert_eq!(reloaded.companies[0].name.as_deref(), Some(name));
        }
        if name == "Transportes del Sur y del Litoral"
            && let Ok(path) = std::env::var("OPENTTDRS_DUMP_RENAMED_NATIVE_SAV")
        {
            std::fs::write(path, output).expect("dump for OpenTTD");
        }
    }
}
