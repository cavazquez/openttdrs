# NewGRF station neighbour scope — issue #390

Updated: 2026-09-06

## Implemented

Station Action2 now has catalogue-aware context constructors used by the
renderer and the animation scheduler:

* `action2_eval_ctx_for_station_tile_with_catalog` and its world-aware variant
  inspect only the `(variable, parameter)` pairs present in the current spec;
* offsets use signed nibbles and toroidal map wrapping, matching
  `GetNearbyTile`;
* `0x66` resolves the animation frame only for a rail tile belonging to the
  same station; `0x67` returns nearby land information with the GRF-version z
  convention;
* `0x68` reports nearby rail gfx, perpendicular/same-station bits and the
  custom-spec identity bits; `0x6A` returns the nearby GRFID and `0x6B` returns
  the local id only when the GRF matches;
* absent, road, airport and vanilla/custom-mismatched neighbours return the
  OpenTTD sentinels instead of leaking the current station's identity.

Both the flat station sprite path and `TileSeq` layout path pass the active
station catalog, as does the CB140–142 station animation context. Legacy
helpers without a catalog keep their existing local-only fallback.

## Regression evidence

`station_ctx_resolves_neighbour_vars_with_wrap_and_grf_identity` covers:

* same-station and different-station neighbours;
* perpendicular axes and custom gfx bits;
* same/different GRFID behavior for `0x6A`/`0x6B`;
* negative/wrapped offsets and absent-tile sentinels;
* coexistence of multiple parameters for the same variable in
  `Action2EvalCtx::parameterized_vars`.

The full core/client test, clippy, format and parity-doc gates remain required
before closing this issue. This scope does not claim sounds, strings, 16-bit
layouts or the complete airport/BaseStation resolver; those remain in #329.
