# Snapshot oracle workflow

Este flujo crea dos snapshots comparables:

- `openttdrs` (loader actual)
- `oráculo` usando un fork mínimo de OpenTTD (wrapper operativo)

## 1) Snapshot desde `.ottdmap` (openttdrs)

```bash
cargo run -p openttdrs-core --bin snapshot_dumper -- tests/fixtures/stationlist-test.ottdmap /tmp/openttdrs.snapshot.json
```

(En local suele usarse `assets/maps/*.ottdmap`, ignorado por git; debe ser `MAP1` generado con `scripts/parse_sav.py`.)

## 2) Bootstrap de fork mínimo OpenTTD

```bash
scripts/setup_openttd_oracle_fork.sh /tmp/openttd-oracle
```

Esto clona OpenTTD en el **commit del manifiesto**
([`docs/parity/openttd-reference.json`](parity/openttd-reference.json), #109)
y agrega `tools/export_snapshot.sh` en una rama local `openttdrs-snapshot-oracle`.

## 3) Snapshot oráculo desde `.sav`

```bash
cd /tmp/openttd-oracle
OPENTTDRS_ROOT=/home/cristian/repos/propios/openttdrs \
  ./tools/export_snapshot.sh /ruta/mapa.sav /tmp/openttd.snapshot.json
```

## 4) Comparación rápida

```bash
diff -u /tmp/openttd.snapshot.json /tmp/openttdrs.snapshot.json
```

Para CI conviene usar un comparador estructural (JSON) y fallar ante mismatch en:

- dimensiones
- hash de alturas
- hash de kind/mapt
- hash rail/road bits
- conteo de componentes industria/estación
