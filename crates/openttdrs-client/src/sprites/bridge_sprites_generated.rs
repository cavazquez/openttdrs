// Generado por scripts/gen_bridge_sprites.py — NO EDITAR A MANO.

use openttdrs_core::{BridgePiece, BridgeType};

/// Sprites de tablero: rear rail X/Y, rear road X/Y, front X/Y, pillar X/Y.
pub struct BridgeDeckSpriteIds {
    pub rear_rail: [u32; 2],
    pub rear_road: [u32; 2],
    pub front: [u32; 2],
    pub pillar: [u32; 2],
}

impl BridgeDeckSpriteIds {
    pub const fn empty() -> Self {
        Self {
            rear_rail: [0, 0],
            rear_road: [0, 0],
            front: [0, 0],
            pillar: [0, 0],
        }
    }

    pub fn rear(&self, rail: bool, axis: usize) -> u32 {
        if rail {
            self.rear_rail[axis]
        } else {
            self.rear_road[axis]
        }
    }

    pub fn atlas_name(sid: u32) -> String {
        match sid {
            2545 => "bridge_wood_rail_y.png".to_string(),
            2546 => "bridge_wood_rail_x.png".to_string(),
            2547 => "bridge_wood_road_y.png".to_string(),
            2548 => "bridge_wood_road_x.png".to_string(),
            2549 => "bridge_wood_y_front.png".to_string(),
            2550 => "bridge_wood_x_front.png".to_string(),
            2551 => "bridge_wood_y_pillar.png".to_string(),
            2552 => "bridge_wood_x_pillar.png".to_string(),
            other => format!("bridge_{other}.png"),
        }
    }
}

/// Offsets NFO (w, h, xrel, yrel) por sprite id.
pub fn bridge_sprite_meta(sid: u32) -> Option<(f32, f32, f32, f32)> {
    match sid {
        2455 => Some((52.0, 44.0, -3.0, -26.0)),
        2456 => Some((36.0, 52.0, -3.0, -34.0)),
        2459 => Some((50.0, 44.0, -1.0, -26.0)),
        2460 => Some((36.0, 51.0, -3.0, -34.0)),
        2463 => Some((52.0, 44.0, -47.0, -26.0)),
        2464 => Some((36.0, 52.0, -31.0, -34.0)),
        2467 => Some((50.0, 44.0, -47.0, -26.0)),
        2468 => Some((36.0, 51.0, -31.0, -34.0)),
        2469 => Some((50.0, 41.0, -23.0, -16.0)),
        2470 => Some((50.0, 57.0, -23.0, -32.0)),
        2471 => Some((50.0, 41.0, -23.0, -16.0)),
        2472 => Some((48.0, 57.0, -23.0, -32.0)),
        2477 => Some((36.0, 26.0, -3.0, -3.0)),
        2478 => Some((36.0, 26.0, -3.0, -3.0)),
        2479 => Some((4.0, 10.0, 29.0, 13.0)),
        2480 => Some((4.0, 10.0, -3.0, -3.0)),
        2481 => Some((36.0, 26.0, -31.0, -3.0)),
        2482 => Some((36.0, 26.0, -31.0, -3.0)),
        2483 => Some((4.0, 10.0, -31.0, 13.0)),
        2484 => Some((4.0, 10.0, 1.0, -3.0)),
        2485 => Some((50.0, 57.0, -23.0, -32.0)),
        2487 => Some((50.0, 57.0, -23.0, -32.0)),
        2488 => Some((50.0, 57.0, -25.0, -32.0)),
        2489 => Some((52.0, 52.0, -3.0, -34.0)),
        2491 => Some((36.0, 26.0, -3.0, -3.0)),
        2493 => Some((48.0, 27.0, -23.0, -2.0)),
        2494 => Some((48.0, 27.0, -23.0, -2.0)),
        2495 => Some((48.0, 27.0, -23.0, -2.0)),
        2496 => Some((48.0, 27.0, -23.0, -2.0)),
        2497 => Some((34.0, 21.0, -31.0, -4.0)),
        2498 => Some((34.0, 21.0, -1.0, -4.0)),
        2499 => Some((49.0, 35.0, -24.0, -10.0)),
        2500 => Some((49.0, 35.0, -23.0, -10.0)),
        2501 => Some((49.0, 35.0, -24.0, -10.0)),
        2502 => Some((49.0, 35.0, -23.0, -10.0)),
        2503 => Some((33.0, 27.0, -32.0, -12.0)),
        2504 => Some((33.0, 27.0, 1.0, -12.0)),
        2505 => Some((36.0, 26.0, -33.0, -4.0)),
        2506 => Some((36.0, 26.0, -1.0, -4.0)),
        2507 => Some((48.0, 49.0, -23.0, -24.0)),
        2508 => Some((48.0, 49.0, -23.0, -24.0)),
        2509 => Some((48.0, 43.0, -23.0, -18.0)),
        2510 => Some((50.0, 42.0, -23.0, -17.0)),
        2511 => Some((50.0, 48.0, -23.0, -23.0)),
        2512 => Some((50.0, 48.0, -23.0, -23.0)),
        2514 => Some((48.0, 49.0, -23.0, -24.0)),
        2515 => Some((48.0, 43.0, -23.0, -18.0)),
        2516 => Some((50.0, 42.0, -23.0, -17.0)),
        2517 => Some((50.0, 48.0, -23.0, -23.0)),
        2518 => Some((50.0, 48.0, -23.0, -23.0)),
        2519 => Some((45.0, 48.0, -40.0, -35.0)),
        2520 => Some((51.0, 48.0, -46.0, -35.0)),
        2521 => Some((50.0, 42.0, -47.0, -29.0)),
        2522 => Some((51.0, 41.0, 1.0, -28.0)),
        2523 => Some((51.0, 46.0, 1.0, -33.0)),
        2524 => Some((45.0, 46.0, 1.0, -33.0)),
        2525 => Some((6.0, 11.0, 0.0, -5.0)),
        2526 => Some((38.0, 27.0, -34.0, -5.0)),
        2527 => Some((38.0, 27.0, -2.0, -5.0)),
        2528 => Some((6.0, 11.0, -4.0, -5.0)),
        2545 => Some((50.0, 42.0, -23.0, -17.0)),
        2546 => Some((50.0, 42.0, -25.0, -17.0)),
        2547 => Some((50.0, 42.0, -23.0, -17.0)),
        2548 => Some((50.0, 42.0, -25.0, -17.0)),
        2549 => Some((46.0, 40.0, -2.0, -26.0)),
        2550 => Some((47.0, 41.0, -43.0, -27.0)),
        2551 => Some((28.0, 22.0, 3.0, -4.0)),
        2552 => Some((28.0, 22.0, -29.0, -4.0)),
        2553 => Some((49.0, 44.0, -24.0, -19.0)),
        2554 => Some((49.0, 44.0, -23.0, -19.0)),
        2555 => Some((49.0, 44.0, -24.0, -19.0)),
        2556 => Some((49.0, 44.0, -23.0, -19.0)),
        2557 => Some((32.0, 35.0, -31.0, -19.0)),
        2558 => Some((32.0, 35.0, 1.0, -19.0)),
        2559 => Some((32.0, 41.0, -29.0, -27.0)),
        2560 => Some((49.0, 42.0, -46.0, -27.0)),
        2561 => Some((47.0, 29.0, -46.0, -14.0)),
        2562 => Some((47.0, 29.0, 1.0, -14.0)),
        2563 => Some((49.0, 42.0, -1.0, -27.0)),
        2564 => Some((32.0, 41.0, -1.0, -27.0)),
        2569 => Some((48.0, 40.0, -23.0, -15.0)),
        2570 => Some((50.0, 40.0, -25.0, -15.0)),
        2571 => Some((50.0, 36.0, -25.0, -11.0)),
        2572 => Some((50.0, 36.0, -23.0, -11.0)),
        2573 => Some((50.0, 40.0, -23.0, -15.0)),
        2574 => Some((48.0, 40.0, -23.0, -15.0)),
        2575 => Some((48.0, 40.0, -23.0, -15.0)),
        2576 => Some((50.0, 40.0, -25.0, -15.0)),
        _ => None,
    }
}

const DECK_TABLE: [[BridgeDeckSpriteIds; 6]; 13] = [
    [
        BridgeDeckSpriteIds {
            rear_rail: [2546, 2545],
            rear_road: [2548, 2547],
            front: [2550, 2549],
            pillar: [2552, 2551],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2546, 2545],
            rear_road: [2548, 2547],
            front: [2550, 2549],
            pillar: [2552, 2551],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2546, 2545],
            rear_road: [2548, 2547],
            front: [2550, 2549],
            pillar: [2552, 2551],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2546, 2545],
            rear_road: [2548, 2547],
            front: [2550, 2549],
            pillar: [2552, 2551],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2546, 2545],
            rear_road: [2548, 2547],
            front: [2550, 2549],
            pillar: [2552, 2551],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2546, 2545],
            rear_road: [2548, 2547],
            front: [2550, 2549],
            pillar: [2552, 2551],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [2505, 2506],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2499, 2500],
            rear_road: [2501, 2502],
            front: [2503, 2504],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2499, 2500],
            rear_road: [2501, 2502],
            front: [2503, 2504],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2499, 2500],
            rear_road: [2501, 2502],
            front: [2503, 2504],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2499, 2500],
            rear_road: [2501, 2502],
            front: [2503, 2504],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2499, 2500],
            rear_road: [2501, 2502],
            front: [2503, 2504],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2499, 2500],
            rear_road: [2501, 2502],
            front: [2503, 2504],
            pillar: [2505, 2506],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2469, 2470],
            rear_road: [2487, 2488],
            front: [2463, 2455],
            pillar: [2481, 2477],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2470, 2469],
            rear_road: [2488, 2487],
            front: [2464, 2456],
            pillar: [2482, 2478],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2472, 2471],
            rear_road: [2488, 2487],
            front: [2468, 2460],
            pillar: [2484, 2480],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2471, 2472],
            rear_road: [2487, 2488],
            front: [2467, 2459],
            pillar: [2483, 2479],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2485, 2494],
            rear_road: [2487, 2488],
            front: [2489, 2497],
            pillar: [2491, 2491],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [0, 0],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2469, 2470],
            rear_road: [2487, 2488],
            front: [2463, 2455],
            pillar: [2481, 2477],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2470, 2469],
            rear_road: [2488, 2487],
            front: [2464, 2456],
            pillar: [2482, 2478],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2472, 2471],
            rear_road: [2488, 2487],
            front: [2468, 2460],
            pillar: [2484, 2480],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2471, 2472],
            rear_road: [2487, 2488],
            front: [2467, 2459],
            pillar: [2483, 2479],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2485, 2494],
            rear_road: [2487, 2488],
            front: [2489, 2497],
            pillar: [2491, 2491],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [0, 0],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2469, 2470],
            rear_road: [2487, 2488],
            front: [2463, 2455],
            pillar: [2481, 2477],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2470, 2469],
            rear_road: [2488, 2487],
            front: [2464, 2456],
            pillar: [2482, 2478],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2472, 2471],
            rear_road: [2488, 2487],
            front: [2468, 2460],
            pillar: [2484, 2480],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2471, 2472],
            rear_road: [2487, 2488],
            front: [2467, 2459],
            pillar: [2483, 2479],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2485, 2494],
            rear_road: [2487, 2488],
            front: [2489, 2497],
            pillar: [2491, 2491],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2493, 2494],
            rear_road: [2495, 2496],
            front: [2497, 2498],
            pillar: [0, 0],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2509, 2510],
            rear_road: [2515, 2516],
            front: [2521, 2522],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2507, 2512],
            rear_road: [2518, 2518],
            front: [2519, 2524],
            pillar: [2525, 2528],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2509, 2510],
            rear_road: [2515, 2516],
            front: [2521, 2522],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2507, 2512],
            rear_road: [2518, 2518],
            front: [2519, 2524],
            pillar: [2525, 2528],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2509, 2510],
            rear_road: [2515, 2516],
            front: [2521, 2522],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2507, 2512],
            rear_road: [2518, 2518],
            front: [2519, 2524],
            pillar: [2525, 2528],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2508, 2511],
            rear_road: [2514, 2517],
            front: [2520, 2523],
            pillar: [2526, 2527],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2553, 2554],
            rear_road: [2555, 2556],
            front: [2557, 2558],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2553, 2554],
            rear_road: [2555, 2556],
            front: [2557, 2558],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2553, 2554],
            rear_road: [2555, 2556],
            front: [2557, 2558],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2553, 2554],
            rear_road: [2555, 2556],
            front: [2557, 2558],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2553, 2554],
            rear_road: [2555, 2556],
            front: [2557, 2558],
            pillar: [2505, 2506],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2553, 2554],
            rear_road: [2555, 2556],
            front: [2557, 2558],
            pillar: [2505, 2506],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2569, 2572],
            rear_road: [2573, 2576],
            front: [2559, 2562],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2571, 2570],
            rear_road: [2575, 2574],
            front: [2561, 2564],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2569, 2572],
            rear_road: [2573, 2576],
            front: [2559, 2562],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2571, 2570],
            rear_road: [2575, 2574],
            front: [2561, 2564],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
    ],
    [
        BridgeDeckSpriteIds {
            rear_rail: [2569, 2572],
            rear_road: [2573, 2576],
            front: [2559, 2562],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2571, 2570],
            rear_road: [2575, 2574],
            front: [2561, 2564],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
        BridgeDeckSpriteIds {
            rear_rail: [2570, 2571],
            rear_road: [2574, 2575],
            front: [2560, 2563],
            pillar: [0, 0],
        },
    ],
];

pub fn bridge_deck_sprite_ids(
    bridge_type: BridgeType,
    piece: BridgePiece,
) -> &'static BridgeDeckSpriteIds {
    let bt = bridge_type.as_u8() as usize;
    let pi = match piece {
        BridgePiece::North => 0,
        BridgePiece::South => 1,
        BridgePiece::InnerNorth => 2,
        BridgePiece::InnerSouth => 3,
        BridgePiece::MiddleOdd => 4,
        BridgePiece::MiddleEven => 5,
    };
    &DECK_TABLE[bt][pi]
}
