# NewGRF station general variables — issue #391

Updated: 2026-09-06

`station_action2` now exposes the general variables that are represented by the
current `Station` model in both map-aware and legacy contexts:

* `0x48` is the native 32-bit accepted-cargo mask for the vanilla cargo IDs;
  custom IDs outside that width are deliberately not aliased into another
  slot;
* `0x82` keeps OpenTTD's station-rating base value (`50`);
* `0x86` is the currently reserved zero flag word;
* `0xF0` encodes `Train`, `TruckStop`, `BusStop`, `Airport`, `Dock` and
  `Waypoint` facilities from `StopKind`.

The existing cargo variables (`0x60`–`0x65`, `0x69`) and tile/neighbor scope
remain unchanged. Regression coverage includes rail, bus, truck, dock, airport
and waypoint stops, their acceptance masks and facility bits.

This is intentionally not a full station resolver: string IDs, build date,
road-stop status and complete `BaseStation` data remain tracked by #329.
Vehicle-history flags are now covered separately by issue #396.
