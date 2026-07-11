//! Mini sparklines de texto para historiales mensuales.

const BLOCKS: &[char] = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Últimas `width` muestras como barras Unicode (vacío si no hay datos).
#[must_use]
pub(crate) fn sparkline_u32(values: &[u32], width: usize) -> String {
    if values.is_empty() || width == 0 {
        return String::new();
    }
    let start = values.len().saturating_sub(width);
    let slice = &values[start..];
    let max = slice.iter().copied().max().unwrap_or(0).max(1);
    slice
        .iter()
        .map(|&v| {
            let idx = ((u64::from(v) * (BLOCKS.len() as u64 - 1)) / u64::from(max)) as usize;
            BLOCKS[idx.min(BLOCKS.len() - 1)]
        })
        .collect()
}

/// Variante con valores con signo (usa valor absoluto para la altura).
#[must_use]
#[allow(dead_code)]
pub(crate) fn sparkline_i64(values: &[i64], width: usize) -> String {
    let abs: Vec<u32> = values
        .iter()
        .map(|v| u32::try_from(v.unsigned_abs().min(u64::from(u32::MAX))).unwrap_or(u32::MAX))
        .collect();
    sparkline_u32(&abs, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_scales_to_blocks() {
        let s = sparkline_u32(&[0, 50, 100], 3);
        assert_eq!(s.chars().count(), 3);
        assert!(s.ends_with('█'));
        assert!(s.starts_with('▁'));
    }
}
