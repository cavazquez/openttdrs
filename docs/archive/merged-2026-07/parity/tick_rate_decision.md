# Tick de simulación (~37 Hz)

**Fuente de verdad:** [ADR 0003](../adr/0003-tick-37hz-openttd.md)
(`OTTD_MILLISECONDS_PER_TICK = 27`, `SIM_TICKS_PER_SECOND ≈ 37.04`).

Complementa el hash/pin de [ADR 0002](../adr/0002-determinismo-tick-referencia.md).
Madurez de paridad: [status.md](status.md).

Código: `economy/time.rs`, `engine/physics.rs` (`REFERENCE_PROGRESS_STEP = 112`),
cliente `simulation.rs` (`SIM_TICK_HZ`).
