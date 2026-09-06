# NewGRF station facilities — issue #393

Updated: 2026-09-06

The shared `StopKind::facilities_mask` now follows OpenTTD's
`StationFacilities` bitset for the scopes that the model represents:

* `RailStation=0x01`, `TruckStop=0x02`, `BusStop=0x04`, `Airport=0x08` and
  `Dock/Buoy=0x10`;
* `RailWaypoint=0x81` (`Train | Waypoint`);
* `RoadWaypoint=0x86` (`TruckStop | BusStop | Waypoint`).

`road_stop_action2` exposes the same `0xF0` value for a placed bus, truck or
waypoint stop. A context without a station remains empty and does not invent
facility bits. Regression tests cover both the station and RoadStop resolvers.
