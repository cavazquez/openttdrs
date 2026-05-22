# Checklist SP2 — Construcción y herramientas

Criterio de cierre del hito **SP2** en solitario.

- **SP2.1–SP2.5:** implementado; casillas automáticas y de código marcadas abajo.
- **SP2.6:** obligatorio para dar SP2 por **cerrado** — sesión manual (abajo); el test de integración preview es opcional.

Marcar al probar; CI de referencia: **mayo 2026**.

Plan maestro: [PLAN_SP2_CONSTRUCCION.md](PLAN_SP2_CONSTRUCCION.md)  
Paradas/sprites: [SP2_PARADAS_Y_ESTACIONES.md](SP2_PARADAS_Y_ESTACIONES.md)

---

## Automático (CI)

```bash
bash scripts/check.sh ci
cargo test -p openttdrs-core command::
cargo test -p openttdrs-client preview::
```

- [x] `bash scripts/check.sh ci` verde
- [x] Tests `command_error_message` para todos los `CommandError`
- [x] Tests `command_would_fail` alineado con `apply_command` (agua, estación, túnel, puente)
- [x] Tests vía (`place_rail` + `mapt`/`m5`), parada bus (`connect_road_stop`), estación tren

---

## SP2.1 — Errores HUD

- [x] `command_error_message` en core
- [x] `push_build_command_error` en clic, drag, paneles
- [x] Mensaje ~5 s en HUD + pitido suave

**Manual**

- [ ] Carretera en agua → mensaje
- [ ] Estación sin transporte adyacente → mensaje
- [ ] Estación duplicada → mensaje

---

## SP2.2 — Preview

- [x] `command_would_fail` reutilizado en preview (`preview/validation.rs`)
- [x] Ghost rojo cuando el comando fallaría

**Manual**

- [ ] Ghost verde en hierba (carretera)
- [ ] Ghost rojo en agua
- [ ] Parada sin carretera → rojo antes del clic

---

## SP2.3 — Transporte

- [x] `place_road_bits` / drag carretera
- [x] `place_rail` con `TrackBits` y `mapt` ferroviario
- [x] Depósito carretera + boca a carretera
- [x] Túnel / puente con validación y mensajes
- [x] Mapa demo procedural (`demo_layout.rs`) con zonas carretera/vía/puente/túnel

**Manual**

- [ ] Drag carretera recta y esquina
- [ ] Vía visible con sprites de raíl (no solo marrón)
- [ ] Depósito + vehículo desde panel
- [ ] Túnel 2 clics; RMB cancela
- [ ] Puente sobre agua del demo

---

## SP2.4 — Estaciones y paneles

- [x] Parada bus / camión: preview cobertura + error sin red
- [x] Conexión carretera ↔ parada (`connect_road_stop`)
- [x] Estación tren: plataforma + vía de fondo + edificio (gfx 2/3)
- [x] Panel vehículo: órdenes, iniciar/detener, vender
- [x] Agregar destino (clic mapa / parada)
- [x] Preview ruta órdenes

**Manual**

- [ ] Parada bus: ramal hacia la carretera (lado correcto tras RMB)
- [ ] Parada camión («Estación» toolbar) distinta de bus (PNG distinto; puede verse similar)
- [ ] Estación tren junto a vía demo
- [ ] Bus con órdenes entre paradas y depósito

---

## SP2.5 — Industrias

- [x] Preview plantilla industria
- [x] Errores + HUD para colocación inválida

**Manual**

- [ ] Ghost mina en hierba; rojo en agua
- [ ] Panel industria al clic

---

## SP2.6 — Sesión 15 min (**pendiente — cierra SP2**)

Comandos:

```bash
# 1) Procedural
cargo run -p openttdrs-client

# 2) Mapa con estaciones del save
OTTDMAP_FILE=tests/fixtures/stationlist-test.ottdmap cargo run -p openttdrs-client
```

Checklist:

1. [ ] **Procedural:** carretera → depósito → bus → parada (boca correcta) → orden circular → vehículo se mueve
2. [ ] **`stationlist-test.ottdmap`:** parada en carretera existente, sin crash; panel vehículo/estación si aplica
3. [ ] **F5** guardar / **F9** cargar: herramientas y órdenes siguen operativas

Cuando las tres estén marcadas, SP2 puede considerarse **cerrado** en [PLAN_SP2_CONSTRUCCION.md](PLAN_SP2_CONSTRUCCION.md).

### Opcional (no bloquea SP2)

- [ ] Test `command_would_fail` vs `apply_command` para `PlaceIndustrySpec` en agua (hoy solo hay casos carretera/estación/túnel/puente)

---

## Fuera de SP2 (siguiente trabajo)

| Tema | Fase |
|------|------|
| Edificios `BUILD_A/B/C` en paradas bus/truck | SP3 |
| Pendientes carretera/vía en slope | SP3 |
| Estaciones tren multi-tesela / techos | SP3 |
| Economía HUD completa (SP1) | SP1 |
