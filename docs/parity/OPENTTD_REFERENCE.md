# Referencia OpenTTD fijada (#109)

Fuente de verdad machine-readable: [`openttd-reference.json`](openttd-reference.json).

Todos los flujos de paridad / extractores / lectura de C++ deben usar el **mismo commit**. No clonar `master` ni hacer `pull` a HEAD móvil.

## Uso

```bash
./scripts/fetch-openttd-reference.sh
git -C reference/openttd-upstream rev-parse HEAD   # debe == commit del manifiesto
```

El script imprime tag, SHA, URL y licencia. Overrides opcionales (solo depuración):

- `OPENTTD_UPSTREAM_URL`
- `OPENTTD_UPSTREAM_COMMIT` (debe ser SHA completo de 40 hex)

## Actualizar la referencia (deliberado)

1. Elegí un tag/release o SHA estable de [OpenTTD/OpenTTD](https://github.com/OpenTTD/OpenTTD).
2. Actualizá `commit`, `tag` y `pinned_at` en `openttd-reference.json`.
3. Corré `./scripts/fetch-openttd-reference.sh` y verificá el SHA.
4. Revisá impacto en docs/parity, extractores Python y citas `archivo:línea`.
5. Abrí un PR que mencione el SHA anterior → nuevo y el motivo (API upstream, bugfix, release).

No regenerar goldens “de pasada” sin documentar el cambio de referencia.

## Oráculo / fork auxiliar

`scripts/setup_openttd_oracle_fork.sh` también clona el commit del manifiesto (no HEAD).

## Licencia

OpenTTD es **GPL-2.0-only** (`license_spdx` en el manifiesto). El clon vive en `reference/openttd-upstream/` (gitignored); no se vendoriza en este repo.
