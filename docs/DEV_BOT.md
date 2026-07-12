# DevBot — sonda headless de carga / descarga / ingresos

**Fecha:** jul 2026  
**Estado:** implementado (módulo opcional, sin rival CPU aún).  
**Relacionado:** [epics/ai_rivals.md](epics/ai_rivals.md), [parity/status.md](parity/status.md), [ROADMAP_JUNCTIONARY.md](ROADMAP_JUNCTIONARY.md)

Herramienta de desarrollo y QA que corre la simulación **sin cliente Bevy** y responde:
¿el vehículo **cargó**, **descargó** y **cuánto ganó**?

---

## Flags

| Flag | Descripción |
|------|-------------|
| `--scenario <nombre>` | Escenario de paridad (`parity::build_scenario`) |
| `--junction <nombre>` | **Alias** de `--scenario` (Junctionary J0) |
| `--vehicle ID` | Id de vehículo a sondear (default 1) |
| `--ticks N` | Ticks máximos (default 12000) |
| `--export-json path` | Guarda la partida en tick 0 (`parity::export_junction_json`) |
| `--out report.json` | Informe JSON de la sonda |
| `--require-delivery` | Exit ≠ 0 si no hubo entrega |
| `--require-signal-wait` | Solo `train_supply`: espera en señal roja |

### Escenarios disponibles

`truck_bay`, `train_line`, `train_supply`, `train_supply_dual`, `train_supply_signal`, `train_signal`, `train_pbs`, `ai_rival_line`, `rail_signals_mixed`, `loan_interest`, `town_growth`, `breakdown`.

Lista runtime: `cargo run -p openttdrs-core --bin dev_bot -- --help`.

### Junctionary (J0)

Hasta existir builders `junction_*` del wiki, `--junction` y `export_junction_json` usan los escenarios de paridad anteriores.

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --junction rail_signals_mixed --export-json save/scenarios/rail_signals_mixed.json

OTTDJSON_LOAD=save/scenarios/rail_signals_mixed.json cargo run -p openttdrs-client
```

En el cliente (DevConsole F3): `scenario list` y `scenario export <nombre> [ruta]`.

---

## Comandos rápidos (copiar/pegar)

Desde la raíz del repo:

```bash
cd openttdrs
```

### Prueba básica — tren `train_line` (recomendada primero)

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_line --vehicle 1 --ticks 12000 --require-delivery
```

Éxito: exit code `0`, JSON con `"loaded": true`, `"delivered": true`, `"delivery_income" > 0`.

### Cadena productor → consumidor — tren `train_supply`

Mina de carbón → estación A → señal en (7,6) → estación B junto a fábrica:

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_supply --vehicle 1 --ticks 12000 --require-delivery
```

Éxito: `"cargo_type": "Coal"`, `loaded` y `delivered` en `true`.

### Doble vía — 1 tren, ida y vuelta (`train_supply_dual`)

| Vía | Hilera | Sentido | Rol |
|-----|--------|---------|-----|
| Ida | y=6 | A → B (este) | Carga carbón en A, descarga en B |
| Vuelta | y=4 | B → A (oeste) | Mismo tren vacío de vuelta |

**Dos rieles físicos separados** (no un solo carril bidireccional). Señales **unidireccionales** por vía: orientación `0` = →este (+x) en y=6, orientación `2` = ←oeste (-x) en y=4. Conectores en x=3 y x=10 para cambiar de vía en los extremos. Solo **2 estaciones** (A ~(1,6), B ~(13,6)).

```bash
# Ciclo completo A → B → A
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_supply_dual --vehicle 1 --ticks 12000 --require-delivery

# Ver en cliente (arranque directo al escenario, sin menú)
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_supply_dual --export-json save/scenarios/train_supply_dual.json

OTTDJSON_LOAD=save/scenarios/train_supply_dual.json cargo run -p openttdrs-client
```

En el mapa: vía de arriba (y=6) solo hacia la derecha; vía de abajo (y=4) solo hacia la izquierda. El pathfinder **YAPF** (`pathfinder/yapf.rs`) elige la vuelta por y=4 sin waypoints artificiales: las señales unidireccionales en contra son callejón sin salida al planificar.

### Probar que las señales funcionan (espera en rojo)

`train_supply` incluye **4 señales** en la L: (5,6), (7,6), (10,6) y (12,8).

Headless — el bot inyecta un tren bloqueador cuando el líder llega a la señal (7,6):

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_supply --require-signal-wait --ticks 12000
```

Éxito en stderr: `bloqueador inyectado: true`, `esperó: true`, `reanudó: true`.

### Ver el escenario en el cliente Bevy

El bot corre **headless** (sin ventana). Usa el **mismo motor** (`GameState`, `sim_step`) que el juego, pero no se integra solo al abrir el cliente.

Exportá la partida al tick 0 y cargala en la ventana:

```bash
mkdir -p save/scenarios
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_supply --export-json save/scenarios/train_supply.json

OTTDJSON_LOAD=save/scenarios/train_supply.json cargo run -p openttdrs-client
```

Con `OTTDJSON_LOAD` el cliente **salta el menú** y entra directo al escenario. (`OPENTTDRS_JSON_SAVE` solo define la ruta por defecto de F5/F9, no carga sola.)

En el cliente: acercá la cámara a la L ferroviaria (estación A ~(1,6), señales en (5,6)/(7,6)/(10,6)/(12,8), estación B ~(12,10)), poné **velocidad** y observá al tren cargar carbón en A y llevarlo a B.

**Instantánea con tren detenido en señal** (bloqueador ya colocado):

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_supply_signal --export-json save/scenarios/train_supply_signal.json

OTTDJSON_LOAD=save/scenarios/train_supply_signal.json cargo run -p openttdrs-client
```

Al quitar el bloqueador (o avanzar la simulación) el tren en (7,6) debería continuar hacia B.

### Guardar informe JSON

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario train_line --vehicle 1 --ticks 12000 \
  --out /tmp/train_cargo.json --require-delivery
```

### Camión — escenario `truck_bay`

```bash
cargo run -p openttdrs-core --bin dev_bot -- \
  --scenario truck_bay --vehicle 1 --ticks 5000 --require-delivery
```

### Tests automáticos del módulo

```bash
cargo test -p openttdrs-core dev_metrics
```

### Check completo del proyecto

```bash
./scripts/check.sh
```

### Ayuda del binario

```bash
cargo run -p openttdrs-core --bin dev_bot -- --help
```

---

## Opciones de `dev_bot`

| Flag | Default | Descripción |
|------|---------|-------------|
| `--scenario <nombre>` | `train_line` | Escenario de `parity::build_scenario` |
| `--vehicle <id>` | `1` | Id del vehículo a observar |
| `--ticks <N>` | `12000` | Máximo de ticks de simulación |
| `--out <archivo.json>` | (stdout) | Guardar informe en disco |
| `--export-json <partida.json>` | off | Exportar `GameState` al tick 0 (cargar con `OTTDJSON_LOAD=…`) |
| `--require-delivery` | off | Exit code `1` si no hubo descarga |
| `--require-signal-wait` | off | Exit code `1` si no hubo espera/reanudación en señal (`train_supply`) |

### Escenarios disponibles

```
truck_bay, train_line, train_supply, train_supply_dual, train_supply_signal, train_signal, loan_interest, town_growth, breakdown
```

Listar en runtime:

```bash
cargo run -p openttdrs-core --bin dev_bot -- --help
```

---

## Campos del informe (`VehicleCargoReport`)

| Campo | Tipo | Significado |
|-------|------|-------------|
| `vehicle_id` | u32 | Vehículo observado |
| `ticks_run` | u64 | Ticks simulados hasta descarga o límite |
| `loaded` | bool | Pasó de `cargo == 0` a `cargo > 0` |
| `delivered` | bool | Tras cargar, descargó (`cargo` → 0 y `cargo_deliveries++`) |
| `cargo_type` | string | Tipo de carga (ej. `"Goods"`) |
| `units_loaded_peak` | u32 | Máximo a bordo tras cargar |
| `units_delivered` | u32 | Unidades entregadas en la primera descarga |
| `delivery_income` | u64 | Ingreso por transporte (`stats.cargo_income_earned` en la ventana) |
| `money_net` | i64 | Δ `economy.money` (incluye costes de explotación) |
| `tick_loaded` | u64? | Tick en que cargó |
| `tick_delivered` | u64? | Tick en que descargó |

### Interpretar `delivery_income` vs `money_net`

- **`delivery_income`**: solo el pago por entregar carga (paridad con OpenTTD).
- **`money_net`**: balance real de la compañía en ese intervalo (puede ser **negativo** si los costes de explotación del tren superan el ingreso del viaje).

Ejemplo real en `train_line` (~146 ticks):

```json
{
  "delivery_income": 14,
  "money_net": -1114,
  "units_loaded_peak": 19,
  "units_delivered": 19
}
```

---

## Uso desde Rust (tests / herramientas)

```rust
use openttdrs_core::{
    dev_metrics::{CargoProbeOptions, probe_vehicle_cargo_cycle},
    parity,
};

let mut state = parity::build_scenario("train_line").unwrap();
let report = probe_vehicle_cargo_cycle(
    &mut state,
    &CargoProbeOptions {
        vehicle_id: 1,
        max_ticks: 12_000,
    },
);
assert!(report.delivered);
assert!(report.delivery_income > 0);
```

API pública en `openttdrs_core::dev_metrics` y reexportada en la raíz del crate.

---

## Arquitectura (módulos)

| Ruta | Rol |
|------|-----|
| `crates/openttdrs-core/src/dev_metrics/` | Lógica de medición (opcional) |
| `crates/openttdrs-core/src/dev_metrics/cargo_probe.rs` | `probe_vehicle_cargo_cycle` |
| `crates/openttdrs-core/src/bin/dev_bot.rs` | CLI |
| `crates/openttdrs-core/src/ai/mod.rs` | Trait `CompanyAi` (vacío, rival futuro) |
| `crates/openttdrs-core/src/parity/scenario.rs` | Escenarios determinísticos |

---

## Cómo eliminar el módulo (si no interesa)

1. Borrar `crates/openttdrs-core/src/dev_metrics/`
2. Borrar `crates/openttdrs-core/src/bin/dev_bot.rs`
3. Borrar `crates/openttdrs-core/src/ai/` (si no hay rival aún)
4. Quitar en `lib.rs`: `pub mod dev_metrics`, `pub mod ai`, y el `pub use dev_metrics::…`
5. Borrar esta doc y la sección en `epics/ai_rivals.md`

No afecta al cliente Bevy ni a partidas guardadas.

---

## Primera tarea recomendada

1. Ejecutar la **prueba básica `train_line`** (comando arriba).
2. Confirmar exit code `0` y `delivered: true`.
3. Si falla → revisar simulación (`sim_step.rs`, escenario en `parity/scenario.rs`).

---

## Siguientes pasos (roadmap)

| # | Tarea | Comando / criterio |
|---|--------|-------------------|
| 1 | ✅ Sonda carga/descarga/ingreso | `dev_bot --scenario train_line --require-delivery` |
| 2 | ✅ Escenario mina→fábrica con señal | `dev_bot --scenario train_supply --require-delivery` |
| 3 | ✅ Exportar partida para el cliente | `dev_bot --export-json save/scenarios/train_supply.json` |
| 4 | Escenario `ai_smoke`: bot construye vía + estación + tren | Nuevo en `parity/scenario.rs` |
| 5 | Política `CompanyAi` mínima (reglas, no ML) | `ai/rule_based.rs` |
| 6 | CI: `dev_bot --require-delivery` tras `check.sh` | `.github/workflows/ci.yml` |
| 7 | Rival jugable multi-compañía | Ver [epics/ai_rivals.md](epics/ai_rivals.md) |

---

## Comparar con `parity_runner`

| Herramienta | Para qué |
|-------------|----------|
| **`dev_bot`** | ¿Cargó? ¿Descargó? ¿Cuánto ganó? (métricas de negocio) |
| **`parity_runner`** | Traza JSONL tick a tick (paridad posición lógica) |

```bash
# Paridad posicional (traza detallada)
cargo run -p openttdrs-core --bin parity_runner -- \
  --scenario train_line --ticks 500 --out /tmp/train_line.jsonl
```

---

## Troubleshooting

| Síntoma | Qué revisar |
|---------|-------------|
| `loaded: false` | Stock en estación/industria; órdenes del vehículo; pathfinder |
| `delivered: false` | Estación destino acepta el cargo; `max_ticks` bajo (subir a 12000) |
| `delivery_income: 0` | `transported_goods_income` en `economy.rs`; distancia fuente→destino |
| Exit code 2 | Argumentos CLI inválidos (`--help`) |
| Compilación lenta | `cargo run -q …` o compilar una vez: `cargo build -p openttdrs-core --bin dev_bot` |

Recompilar tras cambios en core:

```bash
cargo build -p openttdrs-core --bin dev_bot
./target/debug/dev_bot --scenario train_line --require-delivery
```
