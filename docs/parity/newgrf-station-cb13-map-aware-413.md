# NewGRF station CB13 with catalogue and neighbours (#413)

Updated: 2026-09-06.

`CBID_STATION_AVAILABILITY` for an already placed station now has an explicit
catalogue/world-aware entry point:
`apply_station_availability_callback_at_with_catalog_and_world`. It reuses the
same `StationScopeResolver` construction as the station renderer and therefore
does not silently downgrade a real tile to the legacy station-only context.

For a valid station tile the callback can now inspect:

- neighbour parameters `0x66`, `0x67`, `0x68`, `0x6A` and `0x6B`, including the
  neighbouring spec's GRFID and Action3 local ID;
- catalogue-backed badge values (`0x7A`);
- the nearest deterministic `TownScopeResolver` parent, including parent
  variables and the station spec's GRFID-scoped PSA `7C`;
- the existing tile, cargo, owner, PBS and random variables.

The callback still clones the station PSA before evaluation and writes it back
to the same station afterwards. A stale tile/index pair uses the previous
station-only fallback and never clears persistent storage. The original
`apply_station_availability_callback_at` API remains available for callers that
do not own the active station catalogue or world pools.

The regression fixture separates two rail stations by one tile so the
neighbour resolver cannot mistake the adjacent platform for the source
station's flood-filled footprint. It checks a packed `0x68` neighbour result
and a subsequent `7C` writeback through the new entry point.

The complete `BaseStation` scope, GRF strings/sounds and native station-to-town
association are still outside this issue and remain tracked by parent #329.
