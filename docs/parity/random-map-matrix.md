# Paridad de mapas aleatorios

Esta matriz separa dos contratos que suelen confundirse:

1. **Interoperabilidad `.sav`**: OpenTTD genera una partida aleatoria y
   `openttdrs` abre exactamente esa partida.
2. **Generación con el mismo seed**: ambos generadores crean un mapa nuevo
   desde cero con las mismas dimensiones y semilla.

El primer contrato es el que permite abrir mapas de OpenTTD. El segundo exige
portar el algoritmo de `genworld` y no se deduce de que el cargador sea
compatible.

## Método reproducible

La herramienta [`scripts/random_map_parity.py`](../../scripts/random_map_parity.py)
usa OpenTTD headless para generar el mapa y un `.sav` por caso. El hook de
exportación escribe el contrato [`world-raw`](WORLD_RAW_SCHEMA.md) y conserva
el `.sav`; luego `world_raw_dumper` carga el `.sav` real y vuelve a exportar sus
teselas desde Rust. La comparación primaria es exacta sobre `height`, `type` y
`m1..m8`, en orden fila-mayor. Para localizar divergencias se agrupan las
teselas en bloques de 4×4; no se usa una captura de pantalla grande como
criterio de igualdad.

La corrida completa usa muchas semillas en el tamaño mínimo y reduce la
cantidad al crecer:

```text
64×64:   8 mapas
128×128: 4 mapas
256×256: 2 mapas
512×512: 1 mapa
```

Ejemplo, incluyendo el contraste del generador Rust:

```bash
python3 scripts/random_map_parity.py \
  --reference-commit "$(git -C reference/openttd-upstream rev-parse HEAD)" \
  --compare-generator \
  --out-dir /tmp/openttdrs-random-map-matrix \
  --report /tmp/openttdrs-random-map-matrix/report.json \
  --keep-artifacts
```

Sin `--compare-generator` la herramienta sólo valida la apertura del `.sav` y
termina con código cero cuando no hay diferencias. Con esa opción el código de
salida es no cero si el generador procedural Rust no reproduce el mapa de
OpenTTD; eso es una señal de brecha de producto, no un fallo del cargador.

## Resultado observado (15 mapas)

La evidencia compacta está en
[`evidence/random-map-matrix/report.json`](evidence/random-map-matrix/report.json).
El checkout de referencia usado para esta corrida era `c2661164`; el pin
canónico documentado de OpenTTD 15.3 es `14ec60f`. Por esa diferencia, esta
corrida es evidencia diagnóstica local y debe repetirse contra el pin antes de
convertirse en gate de release.

| Tamaño | Casos | Teselas por caso | Bloques 4×4 por caso | Apertura `.sav` | Generador mismo seed |
|---:|---:|---:|---:|:---:|:---:|
| 64×64 | 8 | 4.096 | 256 | **8/8 exactos** | 0/8 exactos; 8/8 divergen |
| 128×128 | 4 | 16.384 | 1.024 | **4/4 exactos** | 0/4 exactos; 4/4 divergen |
| 256×256 | 2 | 65.536 | 4.096 | **2/2 exactos** | 0/2 exactos; 2/2 divergen |
| 512×512 | 1 | 262.144 | 16.384 | **1/1 exacto** | 0/1 exacto; 1/1 diverge |
| **Total** | **15** | — | — | **15/15; 0 teselas y 0 bloques cambiados** | **0/15; 15/15 divergen; 3.846–260.108 teselas distintas por caso; 15/15 bloques completos afectados** |

La divergencia del generador aparece ya en la primera tesela del caso 64×64
(altura y tipo distintos) y no es un problema de resolución de la imagen:
afecta al estado lógico del mapa. El generador Rust ahora porta la escala TGP,
el RNG previo a la normalización, las costas de OpenTTD, `water_borders`, los
bordes `MP_VOID`, el conteo de ríos y la etapa inicial de pueblos/industrias.
Eso corrigió la causa gruesa (el resultado ya no es un heightmap sin población),
pero aún no es una reproducción bit a bit de `GenerateLandscape`/`genworld`:
quedan los ríos y la secuencia exacta de pueblos, industrias, objetos, árboles
y sus bytes `m1..m8`.

## Estado e issues

El registro de trabajo, con criterios de cierre y estado actual, está en
[`random-map-issues.md`](random-map-issues.md). La infraestructura de
generación, exportación, carga y comparación ya está completada. RMAP-004 sigue
siendo el único gap de producto de esta matriz: la **equivalencia del generador
con el mismo seed**. No se marca como resuelto mientras el resultado lógico
siga divergente.
