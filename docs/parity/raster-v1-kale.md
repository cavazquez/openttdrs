# V1-RAS — Kale `(132,2)`

La certificación V1 de raster es un contrato focal, no una declaración de
paridad global de framebuffer. Fija `save/Kale_TitleGame.sav` (SHA-256
`584d98c3d1dc389e938ce92aa357cc4a1c179bf9849133f9b85d2e956f3e0a69`), centro
`(132,2)`, `800×600`, OpenGFX 8bpp, perfil `clean-static` y zoom Normal.

## Resultado certificado

La corrida de 2026-09-20 para la candidata
`48f4d9a9830dfc088961577969082b25b5b02aa9` pasó el gate V1:

| Regla | Resultado | Límite |
|---|---:|---:|
| Píxeles RGBA distintos | 45 | ≤480 |
| Píxeles con delta de canal >64 | 5 | ≤24 |
| Media absoluta por canal RGBA | 0,0024390625 | ≤0,05 |
| Cobertura faltante | 0 | 0 |

Las tres capturas OpenTTD tienen SHA-256
`81f49dd2e245475d6dec35b1ab4d857a49014ddb7526e40d0363b029ce3d4155`; las tres
capturas de openttdrs tienen SHA-256
`6eb808c54266c118374ab882692604af801fd6b1df9380c63acd3f8f88c70eb5`.
El oracle es OpenTTD 15.3 oficial
`14ec60f248547d4d062a1160f0fc26d742319888`, con el exportador instrumentado
en `c2661164bcb6cbf5ab97b56ccbee7506a3b26833`.

El perfil espera **360 frames** antes de capturar. La primera ejecución a 180
frames expuso una transición visual de 392 píxeles entre dos arranques; elevar
la espera fija, sin alinear ni enmascarar, volvió las tres imágenes candidatas
hash-idénticas. Este ajuste es parte del perfil reproducible, no una tolerancia
adicional.

## Evidencia conservada

[`2026-09-20-48f4d9a9/v1-raster-report.json`](evidence/kale-132-2/2026-09-20-48f4d9a9/v1-raster-report.json)
contiene pins, SHA de assets, las tres muestras de cada motor, el diff y los
cuatro checks. Los PNG y logs usados por la corrida están en el mismo
directorio:

- `normal/`: tres referencias, tres candidatas y `diff.png` de Normal.
- `diagnostics/`: una captura y reporte para `In4x`, `In2x`, `Out2x`, `Out4x`
  y `Out8x`; son información de regresión, no criterios verdes.
- `logs/`: comandos y salida de los exportadores de ambos motores.

Los reportes diagnósticos se regeneraron después de copiar los PNG capturados
al árbol versionado únicamente para reemplazar rutas temporales por rutas del
repositorio; sus PNG, hashes, logs y métricas de captura se conservan. El gate
vuelve a verificar esos hashes antes de aceptar el reporte final.

## Reproducción

Con el oracle integrado localmente y un directorio de salida vacío:

```bash
./scripts/capture_v1_raster.sh /tmp/openttdrs-v1-raster
```

El comando falla si la fixture, assets, pin del oracle, cambios rastreados,
geometría, cualquiera de las tres repeticiones, una escala diagnóstica o uno
de los cuatro presupuestos no coincide. No tiene modo `skip`.

Esto satisface el contrato técnico de
[#589](https://github.com/cavazquez/openttdrs/issues/589); su cierre sigue
pendiente de la ejecución verde de CI para el commit que versiona esta
evidencia. No cierra la composición raster global ni convierte los otros cinco
zooms en paridad aceptada.
