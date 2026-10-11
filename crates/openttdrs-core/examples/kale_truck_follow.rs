//! Prepare Kale truck 314, then prove its full service cycle and save continuation.
//! See docs/parity/kale-truck-follow.md. The original SAV and orders are untouched.

#[path = "support/kale_road_follow.rs"]
mod follow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    follow::run(follow::Case {
        id: 314,
        kind: openttdrs_core::VehicleKind::Truck,
        home_order: 1,
        example: "kale_truck_follow",
        label: "truck",
    })
}
