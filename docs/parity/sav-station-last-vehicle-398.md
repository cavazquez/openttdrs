# SAV station last vehicle type — issue #398

Updated: 2026-09-06

The SAV bridge now reads `STNN.normal.last_vehicle_type` for modern and
legacy station rows, hydrates it into `Station.last_vehicle_type`, and emits
the native `VehicleType` byte again when writing `STNN`. `VEH_INVALID` remains
an empty history; train, road, ship, and aircraft retain their native codes.
The model maps the native `VEH_ROAD` family to its existing bus-compatible
variant because OpenTTD does not persist the bus/truck/tram subtype here.

Parser, hydration, writer→parser, and wire-code regressions cover the field.
RoadStop `0xF2`/`0xF3`, vehicle subtypes beyond the native road code, and the
remaining station scopes stay tracked by parent issue #329.
