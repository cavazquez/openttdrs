# Resultado V1 por contrato

La certificación de la issue [#603](https://github.com/cavazquez/openttdrs/issues/603)
se limita a los contratos enumerados en
[`v1-contract-manifest.json`](v1-contract-manifest.json). El agregado no ejecuta
tests ni cierra issues: sólo valida evidencia ya producida para una única SHA
candidata.

Cada resultado JSON debe declarar `contract_id`, `issue`, `candidate_sha`,
`fixture`, `command`, `metric`, `threshold`, `result` y una lista no vacía de
artefactos. Cada artefacto se resuelve relativo al resultado y debe conservar
su SHA-256. La métrica numérica se compara con el operador y valor del umbral.

Un contrato sólo aprueba si su `result` es `passed`, la comparación del umbral
es verdadera, la SHA coincide y todos sus artefactos existen con el hash
declarado. Un resultado ausente, inválido, de otra SHA, `failed`, `not-run`,
`skipped`, `ignored`, `timeout`, `diagnostic`, `excluded` o `not-planned` nunca
cuenta como aprobación. El agregado completo sólo devuelve éxito si **todos**
los contratos del manifiesto aprueban.

## Ejecución

El invocador entrega un resultado por cada ID de
[`v1-contract-manifest.json`](v1-contract-manifest.json), todos producidos para
la misma SHA completa de 40 caracteres. Repetir `--report` para cada ID del
manifiesto (los tres siguientes sólo ilustran su forma):

```sh
python3 scripts/aggregate_v1_reports.py \
  --manifest docs/parity/v1-contract-manifest.json \
  --candidate-sha <sha-candidata> \
  --report V1-VIS=artifacts/v1/V1-VIS/result.json \
  --report V1-NETG=artifacts/v1/V1-NETG/result.json \
  --report V1-REPORT=artifacts/v1/V1-REPORT/result.json \
  --json-out artifacts/v1/aggregate.json \
  --markdown-out artifacts/v1/aggregate.md
```

El JSON de salida tiene `status`, `v1_scoped_green`, `input_errors` y el estado
individual de cada contrato. El Markdown es la vista revisable de esa misma
salida. `global_parity` queda siempre en `not-assessed`: el verde acotado V1 no
recolorea las matrices globales ni puede inferirse de issues cerrados.

Las regresiones sintéticas del agregador se ejecutan con:

```sh
python3 scripts/test_aggregate_v1_reports.py
```

Cubren una entrada completa válida y los rechazos por informe faltante, umbral
incumplido, SHA mezclada, estados no aprobables y artefacto alterado.
