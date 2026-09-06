# RoadStop Action0 cost multipliers (#414)

Updated: 2026-09-06.

RoadStop Action0 property `0x15` is now parsed and retained as the native
`build_cost_multiplier` / `clear_cost_multiplier` byte pair. Both fields
default to `16`, matching `OpenTTD` when a GRF does not provide the property,
and survive catalog application and JSON serialization.

The command path uses the same price categories as upstream:

- bus stops use `PR_BUILD_STATION_BUS` / `PR_CLEAR_STATION_BUS`;
- truck stops use `PR_BUILD_STATION_TRUCK` / `PR_CLEAR_STATION_TRUCK`;
- `GetPrice` shift `-4` makes multiplier `16` equal to the vanilla price.

`PlaceBusStop` and `PlaceTruckStop` charge the selected spec's build multiplier
after the availability preflight. Clearing a placed custom road stop charges
its clear multiplier before removing the tile; stops without a resolvable
custom spec keep the existing generic clear fallback. The regression covers
the Action0 parser/catalog path and a build-then-clear command round trip with
different multipliers.

Bridgeable layout properties `0x13`/`0x14` remain metadata skipped by the port;
they do not affect the cost calculation.
