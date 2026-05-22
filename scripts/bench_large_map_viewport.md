# SP3.6 — mapa 256×256 y viewport

## Cargar mapa grande

Desde la raíz del repo:

```bash
OTTDMAP_FILE=tests/fixtures/stationlist-test.ottdmap cargo run -p openttdrs-client
```

(`tests/fixtures/stationlist-test.ottdmap` — export de `stationlist-test.sav`, 256×256.)

## Comportamiento esperado

- Mapas ≥ 4096 teselas: solo se **generan sprites** en la ventana visible (+ margen ~10 teselas).
- Al **panear** fuera del bloque cargado: un remap (no cada frame); la cámara **no** se resetea.
- **F9** / construcción: remap completo del viewport actual (igual que antes, sin respawn por frame).
- Desactivar culling: `OPENTTDRS_MAP_VIEWPORT_OFF=1`.

## Medir FPS

1. Abrir el mapa 256×256 (comando arriba).
2. Pan/zoom con botón derecho y rueda; mirar FPS en el título de ventana.
3. Comparar con `OPENTTDRS_MAP_VIEWPORT_OFF=1` (genera todas las teselas al cargar/panear).

Objetivo SP3.6: pan/zoom fluidos (sin caída masiva al instanciar ~65k sprites).
