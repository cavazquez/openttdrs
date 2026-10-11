//! Prepare Kale bus 1888, then prove its full service cycle and save continuation.
//! See docs/parity/kale-bus-follow.md. The original SAV and orders are untouched.

#[path = "support/kale_road_follow.rs"]
mod follow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    follow::run(follow::Case {
        id: 1888,
        kind: openttdrs_core::VehicleKind::Bus,
        home_order: 0,
        example: "kale_bus_follow",
        label: "bus",
    })
}
