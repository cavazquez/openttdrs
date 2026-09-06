# NewGRF station airport variables — issue #392

Updated: 2026-09-06

`station_action2` now exposes the airport fields already represented by the
`Station` model in both map-aware and legacy contexts:

* `0xF1` returns the compact TTDPatch type: `0` small, `1` large, `2`
  heliport and `3` oilrig. Action0 `Airports` property `0x0D` is preserved in
  the active catalog and on newly placed/rehydrated NewGRF stations; if that
  metadata is unavailable, the vanilla substitute supplies the fallback.
* `0xF6` returns the low 32 bits of `airport_blocks`.
* `0xF7` returns bits `8..15` of the same block word, matching OpenTTD's
  continuation variable.

Regression coverage exercises a vanilla international airport, a NewGRF
TTDPatch type, non-zero FTA blocks, and both legacy and map-aware contexts.

This cut deliberately does not claim `0xF2`/`0xF3` truck/bus status or `0x8A`
vehicle-history flags: those bitsets are not represented by the current model
and remain in parent issue #329.
