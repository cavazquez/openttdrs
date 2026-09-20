# V1-SAV: una edición ORDL re-guardada por OpenTTD

Contrato acotado de [#588](https://github.com/cavazquez/openttdrs/issues/588).
No pretende certificar todas las variantes de órdenes ni un round-trip `.sav`
universal.

## Caso certificado

Sobre `mvp_openttd_rich.sav`,
`sav_v1_ordl_roundtrip` selecciona de forma determinista una orden `Station`
ya enlazada a un pool `ORDL`. La única mutación pasa por la API pública:

```rust
apply_command(&mut state, &Command::SetSharedOrderAt { ... })
```

La operación cicla su `load_type` con `with_toggled_full_load`, exporta el SAV
editado y deja un manifiesto JSON antes de arrancar el dedicated. OpenTTD 15.3
carga ese input y ejecuta su `save`; el segundo test reimporta el resultado.

El manifiesto fija el SHA del candidato, hashes de input/editado, IDs de
vehículos y estaciones, identidad de la lista ORDL y el snapshot semántico de
todas las órdenes. El fixture está fijado por SHA-256 y el caso por vehículo
`0`, orden `0`, estación `(28,39)` y transición
`load_if_possible → full_load`. Para la orden editada compara de forma exacta
tipo, destino y todos los flags serializables; para las restantes exige igual
orden relativo y contenido. Los índices internos de ORDL pueden renumerarse
al guardar, por lo que el enlace se contrasta mediante sus miembros lógicos de
vehículo, no por un índice incidental.

## Gate obligatorio

La matriz `scripts/validate_sav_openttd_matrix.sh` fija
`OPENTTDRS_REQUIRE_OPENTTD=1` y activa `OPENTTDRS_V1_SAV_ORDL=1` para su único
round-trip. La ausencia del binario, del SAV editado, del output nativo o del
manifiesto falla; no hay una ruta de éxito por `SKIP`.

Para reproducir solamente este caso y conservar los artefactos:

```bash
artifact_dir="$(mktemp -d /tmp/openttdrs-v1-sav.XXXXXX)"
OPENTTDRS_REQUIRE_OPENTTD=1 \
OPENTTDRS_V1_SAV_ORDL=1 \
OPENTTDRS_OTTD_ARTIFACT_DIR="$artifact_dir" \
OPENTTDRS_OTTD_LOG_DIR="$artifact_dir/logs" \
bash scripts/roundtrip_sav_openttd.sh \
  crates/openttdrs-core/tests/fixtures/mvp_openttd_rich.sav
```

El directorio conserva `*.ordl-input.sav`, `*.ordl-edited.sav`,
`*.ordl-resaved.sav`, `*.ordl-evidence.json` y el log del dedicated. El
resultado local queda sujeto a CI remota del SHA candidato antes de cerrar el
issue.

## Exclusiones

Quedan fuera órdenes condicionales, cambios de topología de listas, órdenes
compartidas múltiples, NewGRF y compatibilidad universal de saves. Ninguna de
esas exclusiones permite relajar el caso certificado.
