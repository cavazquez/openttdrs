# Contribuir a openttdrs

Guía corta. Detalle de arquitectura: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Decisiones: [docs/adr/](docs/adr/).

## Antes de abrir un PR

1. `./scripts/doctor.sh` (deps de entorno).
2. Cambios acotados a un issue o ADR; sin refactors colaterales.
3. No regenerar goldens, `*_generated.rs` ni fixtures “de pasada”. Si hace falta:
   - goldens / parse_sav → documentar el motivo en el PR;
   - tablas generadas → [docs/parity/GENERATED_TABLES.md](docs/parity/GENERATED_TABLES.md);
   - pin OpenTTD → [docs/parity/OPENTTD_REFERENCE.md](docs/parity/OPENTTD_REFERENCE.md).
4. Lógica de partida en `openttdrs-core` vía `Command` / `apply_command`. El cliente no debe mutar el mundo de juego fuera de ese canal (inventario: [docs/INVENTARIO_MUTACIONES_CLIENTE.md](docs/INVENTARIO_MUTACIONES_CLIENTE.md)).

## Baseline local (alineado con CI)

```bash
./scripts/check.sh fmt-check
./scripts/check.sh lint          # o: cargo clippy --workspace --all-targets -- -D warnings
./scripts/check.sh test
./scripts/check.sh ci            # núcleo compartido con .github/workflows/ci.yml
```

Solo en GitHub Actions (no obligatorio en cada commit local): `rustdoc`, `cargo audit`, `cargo deny`, cobertura en `main`, fetch OpenTTD para regen de tablas. Cabecera de `scripts/check.sh` y [README](README.md#ci-y-calidad).

## Definition of Done (PR)

- [ ] Scope limitado; sin cambios funcionales accidentales.
- [ ] Tests o checks que cubran el cambio (o justificación de por qué no aplican).
- [ ] Evidencia breve: qué fallaba / qué pasa ahora (comando, log, captura).
- [ ] Docs/ADR actualizados si cambia un contrato.
- [ ] Working tree limpio de regeneraciones no pedidas (`git status`).
- [ ] Plantilla de PR completada ([`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md)).

## ADRs

Decisiones con trade-offs van en `docs/adr/NNNN-titulo.md` (plantilla: [docs/adr/README.md](docs/adr/README.md)). No reescribir ADRs aceptadas: superseder con una nueva.

## Seguridad

Vulnerabilidades: [SECURITY.md](SECURITY.md). No abrir issues públicos con exploits.
