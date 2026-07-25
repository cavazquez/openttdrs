# Checklist SP1 — Ciclo jugable (Sprint 4)

Objetivo: partida de **15–30 minutos** en solitario sin trucos manuales.

**Automático:** `cargo test -p openttdrs-core --test sp1_playable_cycle`  
**Demo procedural:** el mapa de arranque incluye mina, paradas y camión con órdenes (`demo_layout.rs`).

---

## Automático (CI)

```bash
bash scripts/check.sh ci
cargo test -p openttdrs-core --test sp1_playable_cycle
cargo test -p openttdrs-core station::coherence_tests
cargo test -p openttdrs-client vehicle_hud_alert_line
```

- [x] Test integración: mina → paradas → camión → carga → entrega → ingresos
- [x] Coherencia `state.stations` ↔ tiles `MP_STATION` (`station_map_coherence`)
- [x] HUD: sin ruta, sin órdenes, parada incompatible, sin carga disponible
- [x] SFX cableados: error (`hud_soft.wav`), construcción OK (`build_ok.wav`), ingreso (`income.wav`)

---

## Guion manual (~15 min)

```bash
cargo run -p openttdrs-client
```

1. [ ] Observar camión demo: carga en mina, entrega en parada lejana, dinero sube en HUD
2. [ ] Colocar carretera + depósito + comprar bus; parada bus con boca correcta (RMB orienta)
3. [ ] Órdenes circulares bus; ver alerta «sin ruta» si falta red
4. [ ] Estación tren 3×2 junto a vía demo; tren con 2 órdenes
5. [ ] **F5** guardar → reiniciar cliente → **F9** cargar; herramientas y órdenes operativas

---

## Criterio de cierre SP1

- Sesión manual sin bugs bloqueantes en los pasos anteriores
- Economía visible (dinero, entregas, texto `+$N` y sonido de ingreso)
- Alertas HUD accionables antes de abrir panel de vehículo

*Última actualización: 2026-06-22*
