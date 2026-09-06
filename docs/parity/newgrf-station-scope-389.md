# NewGRF station scope — issue #389

Updated: 2026-09-06

## What was missing

`CBID_STATION_AVAILABILITY` (`0x13`) had two different call paths:

* construction correctly used the OpenTTD null-station resolver, but
  `apply_station_availability_callback` (the stateful/legacy API) only copied
  random bits and persistent registers into `Action2EvalCtx`;
* the tile renderer and station animation scheduler already had a rich
  map-aware context, but availability had no equivalent public call site.

Consequently a callback reading owner, PBS, platform or animation variables
could silently see zero when invoked through the stateful API.

## Implemented contract

* `action2_eval_ctx_from_station` now exposes the no-tile OpenTTD sentinels for
  `0x40`, `0x41`, `0x46`, `0x47`, `0x49`, `0x42`, `0x44` and unavailable
  `0x45`, plus station owner `0x43`, frame `0x4A`, random bits and pending
  random triggers in `0x5F`.
* `apply_station_availability_callback_at` evaluates an existing station with
  the real map/tile context used by `station_action2`: platform geometry,
  terrain/rail type, PBS/reservation, rail continuation, centered platform
  variants, frame, land info and cargo variables are available to Action2.
* The callback writes `7C` back to the selected `Station` only after
  evaluation. A stale tile/index pair falls back to the legacy context and
  cannot erase the station PSA.
* Construction remains intentionally separate: OpenTTD evaluates CB13 before
  a `Station` or tile exists, so `apply_station_availability_callback_for_build`
  keeps the null-station semantics and has no persistent writeback.

## Evidence

The core regressions are:

* `station_availability_legacy_scope_exposes_station_vars`
* `station_availability_map_context_exposes_tile_scope_and_psa`
* `station_dynamic_vars_share_action2_eval_ctx_for_228`

The first test proves the legacy path no longer drops station variables; the
second verifies owner encoding and `7C` writeback through the map-aware path.
The existing station Action2 suite continues to cover platform geometry,
PBS, rail continuation, frame, cargo variables and random-trigger state.

## Deliberate remaining gap

This issue does not claim full station parity. Neighboring station variables
(`0x66`, `0x68`, `0x6A`, `0x6B`), GRF strings, sounds, 16-bit layouts and
complete `BaseStation`/airport scopes remain in #329. The construction path
also remains a null scope by design, matching OpenTTD.
