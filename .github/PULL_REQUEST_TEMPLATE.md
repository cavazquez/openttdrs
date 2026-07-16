## Resumen

<!-- Qué cambia y por qué (1–3 frases). Enlazar issue: Fixes #N -->

## Tipo

- [ ] Feature / paridad
- [ ] Bugfix
- [ ] Refactor (sin cambio funcional)
- [ ] Tooling / CI / docs
- [ ] ADR / gobierno

## Evidencia

**Antes:**

<!-- comando que fallaba, sintoma, o “n/a” -->

**Después:**

<!-- mismo comando en verde, o captura / hash / log corto -->

```bash
# pegá aquí el comando de verificación principal
```

## Checklist

- [ ] Scope acotado; sin regenerar goldens/`*_generated.rs` salvo que el PR lo justifique
- [ ] `./scripts/check.sh ci` (o el subconjunto relevante) en verde en local
- [ ] Tests nuevos/actualizados si hay lógica de simulación
- [ ] Docs o ADR actualizados si cambia un contrato
- [ ] Sin secretos ni assets no redistribuibles

## Notas para revisores

<!-- riesgos, follow-ups, capturas opcionales -->
