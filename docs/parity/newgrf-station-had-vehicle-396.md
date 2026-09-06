# NewGRF station vehicle history — issue #396

Updated: 2026-09-06

OpenTTD's `StationScopeResolver` exposes `Station::had_vehicle_of_type` in
variable `0x8A`. The core model now keeps the same bitset in `Station`:

* `0x02` train, `0x04` bus, `0x08` truck, `0x10` aircraft and `0x20` ship;
* `0x40` is added for rail/road/ship waypoints, matching `HVOT_WAYPOINT`.

The bitset is updated when a vehicle can actually service a station for load or
unload, and `note_station_load_attempt` also records the type used by the
station rating path. Both map-aware and legacy Action2 contexts read the same
value. The field has `serde(default)`, so old JSON saves load with an empty
history and new JSON saves round-trip every bit.

Native SAV parity for `STNN.normal.had_vehicle_of_type` is covered separately
by issue #397. The road-stop status variables `0xF2`/`0xF3` remain tracked by
parent issue #329.
