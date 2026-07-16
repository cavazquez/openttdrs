# Arquitectura (resumen)

Contrato actual del repo. Paridad detallada y roadmaps: [docs/README.md](README.md). Upstream OpenTTD (referencia, no este código): [INFORME_ARQUITECTURA_OPENTTD.md](INFORME_ARQUITECTURA_OPENTTD.md).

## Capas

```text
┌─────────────────────────────────────────┐
│  openttdrs-client (Bevy)                │  presentación, input, assets
│  — UI, render, bootstrap, --server/--client
└─────────────────┬───────────────────────┘
                  │ Command / apply_command_log / tick
┌─────────────────▼───────────────────────┐
│  openttdrs-core                         │  simulación pura
│  — mapa, vehículos, economía, saves, hash
└─────────────────┬───────────────────────┘
                  │
┌─────────────────▼───────────────────────┐
│  openttdrs-net (+ bin dedicated)        │  transporte TCP lockstep
└─────────────────────────────────────────┘
```

| Crate | Rol |
|-------|-----|
| `openttdrs-core` | Estado de partida, `Command`, `GameState::step`, `canonical_hash`, import/export |
| `openttdrs-client` | Adaptador Bevy: no es fuente de verdad del mundo |
| `openttdrs-net` | Framing del log de comandos; listen-server / cliente / dedicated |

## Reglas duras

1. **Mutación de partida** → `Command` (o load/tick orquestado). Ver [INVENTARIO_MUTACIONES_CLIENTE.md](INVENTARIO_MUTACIONES_CLIENTE.md).
2. **Determinismo** → mismo seed + mismos comandos + mismos ticks ⇒ mismo `canonical_hash` ([ADR 0002](adr/0002-determinismo-tick-referencia.md)).
3. **Referencia OpenTTD** → commit fijado en [parity/openttd-reference.json](parity/openttd-reference.json); no clonar `master` móvil.
4. **Red** → lockstep TCP ([ADR 0001](adr/0001-multiplayer-v1.md)); host migration listen-server post-v1 ([ADR 0004](adr/0004-host-migration-post-v1.md)).

## Dónde va código nuevo

| Quiero… | Va en… |
|---------|--------|
| Regla de simulación / save / paridad headless | `openttdrs-core` |
| Ventana, sprite, input, menú | `openttdrs-client` |
| Protocolo / dedicated | `openttdrs-net` / bin dedicated |
| Decisión con trade-off | `docs/adr/` |
