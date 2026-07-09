# Referencia Completa de Sprites OpenGFX

Este documento cataloga **todos** los sprites de OpenGFX relevantes para el renderizado de
tiles en openttdrs, basándose en el archivo `sprites.h` de OpenTTD y el NFO de OpenGFX 8.0.

> **Fuente**: `src/table/sprites.h` del repositorio OpenTTD (GPLv2).
> Todos los sprites con ID < 5126 están en los archivos base (`ogfx1_base.grf`).

---

## Organización del Archivo NFO

El archivo `ogfx1_base.nfo` define cada sprite con el formato:

```
<ID> sprites/ogfx1_base00.png 8bpp <X> <Y> <W> <H> <XREL> <YREL> [flags]
```

- **ID**: número de sprite (usado en `sprites.h`)
- **X, Y**: posición en la hoja PNG
- **W, H**: dimensiones en píxeles
- **XREL, YREL**: offset para anclar el sprite (Y-down en NFO; Bevy usa Y-up)

---

## 1. Terreno Base (MP_CLEAR)

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_FLAT_BARE_LAND` | 3924 | Tierra desnuda plana |
| `SPR_FLAT_1_THIRD_GRASS_TILE` | 3943 | Hierba parcial (1/3) |
| `SPR_FLAT_2_THIRD_GRASS_TILE` | 3962 | Hierba parcial (2/3) |
| `SPR_FLAT_GRASS_TILE` | 3981 | Hierba completa |
| `SPR_FLAT_ROUGH_LAND` | 4000 | Terreno irregular |
| `SPR_FLAT_ROUGH_LAND_1..4` | 4019–4022 | Variantes rough |
| `SPR_FLAT_ROCKY_LAND_1` | 4023 | Terreno rocoso 1 |
| `SPR_FLAT_ROCKY_LAND_2` | 4042 | Terreno rocoso 2 |
| `SPR_FLAT_1_QUART_SNOW_DESERT_TILE` | 4493 | Nieve/desierto 1/4 |
| `SPR_FLAT_2_QUART_SNOW_DESERT_TILE` | 4512 | Nieve/desierto 2/4 |
| `SPR_FLAT_3_QUART_SNOW_DESERT_TILE` | 4531 | Nieve/desierto 3/4 |
| `SPR_FLAT_SNOW_DESERT_TILE` | 4550 | Nieve/desierto completo |

### Pendientes

Los sprites de pendiente se calculan como `SPR_FLAT_* + tileh` donde `tileh` es la
máscara de elevación de esquinas (1–14 para pendientes simples).

---

## 2. Agua (MP_WATER)

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_FLAT_WATER_TILE` | 4061 | Agua plana |
| `SPR_ORIGINALSHORE_START` | 4062 | Costas originales (4062–4069) |
| `SPR_SHORE_BASE` | calculado* | Costas nuevas (18 sprites) |

*`SPR_SHORE_BASE = SPR_2CCMAP_BASE + 256` (depende de versión)

### Ship Depot

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_SHIP_DEPOT_SE_FRONT` | 4070 | Frente SE |
| `SPR_SHIP_DEPOT_SW_FRONT` | 4071 | Frente SW |
| `SPR_SHIP_DEPOT_NW` | 4072 | NW |
| `SPR_SHIP_DEPOT_NE` | 4073 | NE |
| `SPR_SHIP_DEPOT_SE_REAR` | 4074 | Trasero SE |
| `SPR_SHIP_DEPOT_SW_REAR` | 4075 | Trasero SW |
| `SPR_BUOY` | 4076 | Boya |

---

## 3. Carreteras (MP_ROAD)

### Carreteras Planas

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_ROAD_Y` | 1332 | Carretera eje Y (NW-SE) |
| `SPR_ROAD_X` | 1333 | Carretera eje X (NE-SW) |
| (1334–1350) | — | Esquinas, T, cruces |

#### Tabla `GetRoadSpriteOffset` (plano)

```rust
const ROAD_OFFSET: [u8; 16] = [
    0, 18, 17, 7,   // bits 0000-0011
    16, 0, 10, 5,   // bits 0100-0111
    15, 8, 1, 4,    // bits 1000-1011
    9, 3, 6, 2      // bits 1100-1111
];
// Sprite final = SPR_ROAD_Y (1332) + ROAD_OFFSET[road_bits]
```

### Pendientes

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_ROAD_SLOPE_START` | 1343 | Base para pendientes |
| `SPR_ROAD_Y_SNOW` | 1351 | Y con nieve |
| `SPR_ROAD_X_SNOW` | 1352 | X con nieve |

### Depósito de Carretera

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_ROAD_DEPOT` | 1408 | Depósito (4 direcciones) |

---

## 4. Vías Férreas (MP_RAILWAY)

### Vías Combinadas (suelo + raíles)

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_RAIL_TRACK_Y` | 1011 | Vía eje Y |
| `SPR_RAIL_TRACK_X` | 1012 | Vía eje X |
| (1013–1016) | — | UPPER, LOWER, RIGHT, LEFT |
| (1017) | — | Cruce X+Y |
| `SPR_RAIL_TRACK_BASE` | 1018 | Base para junctions (5 sprites) |
| `SPR_RAIL_TRACK_N_S` | 1035 | Vía HORZ (upper+lower) |
| (1036) | — | Vía VERT (left+right) |

### Piezas Sueltas (para junctions)

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_RAIL_SINGLE_X` | 1005 | Overlay X |
| `SPR_RAIL_SINGLE_Y` | 1006 | Overlay Y |
| `SPR_RAIL_SINGLE_NORTH` | 1007 | Overlay UPPER |
| `SPR_RAIL_SINGLE_SOUTH` | 1008 | Overlay LOWER |
| `SPR_RAIL_SINGLE_EAST` | 1009 | Overlay RIGHT |
| `SPR_RAIL_SINGLE_WEST` | 1010 | Overlay LEFT |

### Depósitos de Tren

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_RAIL_DEPOT_SE_1` | 1063 | Depósito SE parte 1 |
| `SPR_RAIL_DEPOT_SE_2` | 1064 | Depósito SE parte 2 |
| `SPR_RAIL_DEPOT_SW_1` | 1065 | Depósito SW parte 1 |
| `SPR_RAIL_DEPOT_SW_2` | 1066 | Depósito SW parte 2 |
| `SPR_RAIL_DEPOT_NE` | 1067 | Depósito NE |
| `SPR_RAIL_DEPOT_NW` | 1068 | Depósito NW |

### Catenaria eléctrica

| Rango | Uso |
|-------|-----|
| 1039–1062 | Wires OpenGFX (`WSO_*`); plano X/Y alterna SW/NE; pendientes: UP/DOWN 1043–1046; EW/NS=1041/1042 |

### Monorraíl y Maglev

| Tipo | Track Y | Track X | Track Base | N_S |
|------|---------|---------|------------|-----|
| Mono | 1093 | 1094 | 1100 | 1117 |
| Maglev | 1175 | 1176 | 1182 | 1199 |

---

## 5. Estaciones de Tren (MP_STATION)

### Plataformas

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_RAIL_PLATFORM_Y_FRONT` | 1069 | Plataforma Y frente |
| `SPR_RAIL_PLATFORM_X_REAR` | 1070 | Plataforma X trasero |
| `SPR_RAIL_PLATFORM_Y_REAR` | 1071 | Plataforma Y trasero |
| `SPR_RAIL_PLATFORM_X_FRONT` | 1072 | Plataforma X frente |
| `SPR_RAIL_PLATFORM_BUILDING_X` | 1073 | Edificio estación X |
| `SPR_RAIL_PLATFORM_BUILDING_Y` | 1074 | Edificio estación Y |
| `SPR_RAIL_PLATFORM_PILLARS_Y_FRONT` | 1075 | Pilares Y frente |
| `SPR_RAIL_PLATFORM_PILLARS_X_REAR` | 1076 | Pilares X trasero |
| `SPR_RAIL_PLATFORM_PILLARS_Y_REAR` | 1077 | Pilares Y trasero |
| `SPR_RAIL_PLATFORM_PILLARS_X_FRONT` | 1078 | Pilares X frente |

### Techos

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_RAIL_ROOF_STRUCTURE_X_TILE_A` | 1079 | Estructura techo X mitad A |
| `SPR_RAIL_ROOF_STRUCTURE_Y_TILE_A` | 1080 | Estructura techo Y mitad A |
| `SPR_RAIL_ROOF_STRUCTURE_X_TILE_B` | 1081 | Estructura techo X mitad B |
| `SPR_RAIL_ROOF_STRUCTURE_Y_TILE_B` | 1082 | Estructura techo Y mitad B |
| `SPR_RAIL_ROOF_GLASS_X_TILE_A` | 1083 | Cristal techo X mitad A |
| `SPR_RAIL_ROOF_GLASS_Y_TILE_A` | 1084 | Cristal techo Y mitad A |
| `SPR_RAIL_ROOF_GLASS_X_TILE_B` | 1085 | Cristal techo X mitad B |
| `SPR_RAIL_ROOF_GLASS_Y_TILE_B` | 1086 | Cristal techo Y mitad B |

---

## 6. Paradas de Carretera (Bus/Truck)

### Paradas de Bus

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_BUS_STOP_NE_GROUND` | 2692 | Suelo NE |
| `SPR_BUS_STOP_SE_GROUND` | 2693 | Suelo SE |
| `SPR_BUS_STOP_SW_GROUND` | 2694 | Suelo SW |
| `SPR_BUS_STOP_NW_GROUND` | 2695 | Suelo NW |
| `SPR_BUS_STOP_NE_BUILD_A` | 2696 | Edificio A NE |
| `SPR_BUS_STOP_SE_BUILD_A` | 2697 | Edificio A SE |
| `SPR_BUS_STOP_SW_BUILD_A` | 2698 | Edificio A SW |
| `SPR_BUS_STOP_NW_BUILD_A` | 2699 | Edificio A NW |
| `SPR_BUS_STOP_*_BUILD_B` | 2700–2703 | Edificio B (4 dirs) |
| `SPR_BUS_STOP_*_BUILD_C` | 2704–2707 | Edificio C (4 dirs) |

### Paradas de Camión

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_TRUCK_STOP_NE_GROUND` | 2708 | Suelo NE |
| `SPR_TRUCK_STOP_SE_GROUND` | 2709 | Suelo SE |
| `SPR_TRUCK_STOP_SW_GROUND` | 2710 | Suelo SW |
| `SPR_TRUCK_STOP_NW_GROUND` | 2711 | Suelo NW |
| `SPR_TRUCK_STOP_*_BUILD_A` | 2712–2715 | Edificio A (4 dirs) |
| `SPR_TRUCK_STOP_*_BUILD_B` | 2716–2719 | Edificio B (4 dirs) |
| `SPR_TRUCK_STOP_*_BUILD_C` | 2720–2723 | Edificio C (4 dirs) |

### Uso en openttdrs (SP3 paradas)

| Herramienta UI | PNG en cliente | IDs | Render actual |
|----------------|----------------|-----|---------------|
| Parada de bus | `bus_stop_*_ground.png` (+ `build_*` en disco) | 2692–2707 | GROUND + tramo `road_flat` en `m3` |
| Estación (camión) | `truck_stop_ground_{0..3}.png` | 2708–2723 | Igual que bus |
| Estación de tren | `rail_{1069..1074}.png` | 1069–1074 | Vía 1011/1012 + plataformas |

Edificios `build_*`: en OpenTTD van con `TILE_SEQ_LINE` + `RemapCoords` (`station_land.h`); en openttdrs aún no se pintan. Detalle: [archive/PLAN_PARADAS_REMAPCOORDS.md](archive/PLAN_PARADAS_REMAPCOORDS.md).

**No** comparten el mismo sprite bus y camión; pueden verse parecidos al dibujar solo la baldosa.

Documentación: [SP2_PARADAS_Y_ESTACIONES.md](SP2_PARADAS_Y_ESTACIONES.md).

---

## 7. Casas Urbanas (MP_HOUSE)

Las casas tienen sprites para cada etapa de construcción (CNST1, CNST2, CNST3),
un suelo (GROUND) y el edificio terminado (BUILD).

### Edificios Clima Templado

| Tipo | Ground | Build | Notas |
|------|--------|-------|-------|
| Tall Office | 1424 | 1425 | 64×37 / 65×71 |
| Office 01 | 1429 | 1428 | — |
| Small Block Flats | 1433 | 1432 | — |
| Church | 1437 | 1436 | — |
| Large Office | — | 1442 | También ártico/subtropical |
| Town House V1 | 1447 | 1446 | — |
| Town House V2 | 1505 | 1506 | — |
| Hotel NW | — | 1450 | 2 tiles |
| Hotel SE | — | 1453 | 2 tiles |
| Shop/Office | — | 1460, 1463, 1466 | Varias variantes |

### Elementos Decorativos

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_STATUE_HORSERIDER_09` | 1454 | Estatua ecuestre |
| `SPR_FOUNTAIN_0A` | 1455 | Fuente |
| `SPR_PARKSTATUE_0B` | 1456 | Estatua de parque |
| `SPR_PARKALLEY_0C` | 1457 | Callejón de parque |

### Estadio

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_GRND_STADIUM_N` | 1479 | Suelo norte |
| `SPR_GRND_STADIUM_E` | 1480 | Suelo este |
| `SPR_GRND_STADIUM_W` | 1481 | Suelo oeste |
| `SPR_GRND_STADIUM_S` | 1482 | Suelo sur |

---

## 8. Árboles (MP_TREES)

Los árboles no tienen IDs constantes en `sprites.h` pero están en rangos conocidos.
En OpenGFX, los árboles templados empiezan alrededor del sprite **1576**.

| Rango | Clima | Descripción |
|-------|-------|-------------|
| 1576–1617 | Templado | ~7 tipos × 6 etapas |
| 1618–1659 | Ártico | Coníferas y similares |
| 1660–1701 | Subtropical | Palmeras, etc. |
| 1702–1755 | Toyland | Árboles de fantasía |

---

## 9. Industrias (MP_INDUSTRY)

### Coal Mine (Mina de Carbón)

| Constante | ID | Descripción |
|-----------|-----|-------------|
| — | 2013 | Headframe principal (58×50) |
| — | 2028 | Torre alternativa |
| — | 2011 | Entrada |

### Power Plant

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_IT_POWER_PLANT_TRANSFORMERS` | 2054 | Transformadores |

(Las industrias son complejas; cada tipo tiene múltiples sprites
organizados en la tabla `_industry_draw_tile_data` en `industry_land.h`.)

---

## 10. Aeropuertos

### Elementos Base

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_HELIPORT` | 2633 | Helipuerto |
| `SPR_AIRPORT_APRON` | 2634 | Plataforma |
| `SPR_AIRPORT_AIRCRAFT_STAND` | 2635 | Puesto de avión |
| `SPR_AIRPORT_TAXIWAY_*` | 2636–2644 | Calles de rodaje |
| `SPR_AIRPORT_RUNWAY_*` | 2645–2649 | Pista |
| `SPR_AIRPORT_TERMINAL_*` | 2650, 2653, 2654 | Terminales |
| `SPR_AIRPORT_TOWER` | 2651 | Torre de control |
| `SPR_AIRPORT_CONCOURSE` | 2652 | Vestíbulo |
| `SPR_AIRPORT_HANGAR_*` | 2655, 2656 | Hangar |
| `SPR_AIRPORT_RADAR_*` | 2680–2691 | Radar (animado) |

---

## 11. Muelles

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_DOCK_SLOPE_NE` | 2727 | Pendiente NE |
| `SPR_DOCK_SLOPE_SE` | 2728 | Pendiente SE |
| `SPR_DOCK_SLOPE_SW` | 2729 | Pendiente SW |
| `SPR_DOCK_SLOPE_NW` | 2730 | Pendiente NW |
| `SPR_DOCK_FLAT_X` | 2731 | Plano X (NE/SW) |
| `SPR_DOCK_FLAT_Y` | 2732 | Plano Y (NW/SE) |

---

## 12. Puentes y Túneles

### Entrada de Túnel

| Tipo | ID | Descripción |
|------|-----|-------------|
| Rail | 2365 | `SPR_TUNNEL_ENTRY_REAR_RAIL` |
| Mono | 2373 | `SPR_TUNNEL_ENTRY_REAR_MONO` |
| Maglev | 2381 | `SPR_TUNNEL_ENTRY_REAR_MAGLEV` |
| Road | 2389 | `SPR_TUNNEL_ENTRY_REAR_ROAD` |

### Puente de Madera (tipo 0)

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_BTWDN_RAIL_Y_REAR` | 2545 | Rail Y trasero |
| `SPR_BTWDN_RAIL_X_REAR` | 2546 | Rail X trasero |
| `SPR_BTWDN_ROAD_Y_REAR` | 2547 | Road Y trasero |
| `SPR_BTWDN_ROAD_X_REAR` | 2548 | Road X trasero |
| `SPR_BTWDN_Y_FRONT` | 2549 | Frente Y |
| `SPR_BTWDN_X_FRONT` | 2550 | Frente X |
| `SPR_BTWDN_Y_PILLAR` | 2551 | Pilar Y |
| `SPR_BTWDN_X_PILLAR` | 2552 | Pilar X |

(Hay más tipos de puente: Steel Girder, Suspension, etc. con rangos similares.)

---

## 13. Cruces a Nivel

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_CROSSING_OFF_X_RAIL` | 1370 | Rail cruce (eje vía X); `rail_1370.png` en el cliente |
| (siguiente eje vía Y) | 1371 | Par del cruce; `rail_1371.png` (`base_sprites.crossing + rail_axis`) |
| `SPR_CROSSING_OFF_X_MONO` | 1382 | Mono cruce X |
| `SPR_CROSSING_OFF_X_MAGLEV` | 1394 | Maglev cruce X |

---

## 14. Señales

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_ORIGINAL_SIGNALS_BASE` | 1275 | Base señales originales |
| `SPR_SIGNALS_BASE` | calculado | Base señales nuevas (presignals, PBS) |

---

## 15. Objetos Especiales

### Sede de Empresa (HQ)

| Tamaño | IDs | Descripción |
|--------|-----|-------------|
| Tiny | 2603–2606 | 1 tile |
| Small | 2607–2610 | 1 tile |
| Medium | 2611–2617 | 1 tile con paredes |
| Large | 2618–2624 | 2×2 tiles |
| Huge | 2625–2631 | 2×2 tiles |

### Otros

| Constante | ID | Descripción |
|-----------|-----|-------------|
| `SPR_TRANSMITTER` | 2601 | Transmisor |
| `SPR_LIGHTHOUSE` | 2602 | Faro |
| `SPR_STATUE_COMPANY` | 2632 | Estatua de empresa |
| `SPR_BOUGHT_LAND` | 4790 | Terreno comprado |
| `SPR_CONCRETE_GROUND` | 1420 | Suelo de hormigón |

---

## Rangos Importantes (Resumen)

| Categoría | Rango de IDs |
|-----------|--------------|
| Terreno base | 3924–4022, 4493–4550 |
| Agua | 4061–4076 |
| Carreteras | 1332–1352, 1408 |
| Vías (rail) | 1005–1086 |
| Vías (mono) | 1087–1120 |
| Vías (maglev) | 1169–1199 |
| Estaciones tren | 1069–1086 |
| Paradas bus/truck | 2692–2723 |
| Casas | 1421–1510 |
| Árboles | 1576–1755 |
| Aeropuertos | 2633–2691 |
| Muelles | 2727–2732 |
| Puentes | 2453–2552, 4324–4401 |
| Señales | 1275–1330 |
| HQ/Objetos | 2601–2632 |

---

## Uso en openttdrs

El script `scripts/descargar_graficos.sh` extrae sprites del NFO usando estas constantes.
Para agregar nuevos sprites:

1. Buscar el ID en esta referencia o en `sprites.h`
2. Añadir al script: `crop_by_id(<ID>, "<nombre>.png")`
3. Cargar en `main.rs` con `asset_server.load()`
4. Usar según `TileKind` y bytes `m5`/`mapt`

---

## Sistema de Coordenadas y Direcciones

### Direcciones Diagonales (DiagDirection)

OpenTTD usa un sistema de coordenadas donde los ejes están rotados 45° respecto a la pantalla.
Las direcciones diagonales y sus offsets de tile son:

| DiagDirection | Valor | Offset (x, y) | En pantalla |
|---------------|-------|---------------|-------------|
| `DIAGDIR_NE` | 0 | **(-1, 0)** | Arriba-derecha |
| `DIAGDIR_SE` | 1 | **(0, +1)** | Abajo-derecha |
| `DIAGDIR_SW` | 2 | **(+1, 0)** | Abajo-izquierda |
| `DIAGDIR_NW` | 3 | **(0, -1)** | Arriba-izquierda |

> **Fuente**: `src/map.cpp` → `_tileoffs_by_diagdir[]`

### RoadBits

Los `RoadBits` indican hacia qué bordes del tile se extiende una carretera:

| RoadBit | Bit | Valor | Dirección |
|---------|-----|-------|-----------|
| `NW` | 0 | 1 | Hacia tile en (0, -1) |
| `SW` | 1 | 2 | Hacia tile en (+1, 0) |
| `SE` | 2 | 4 | Hacia tile en (0, +1) |
| `NE` | 3 | 8 | Hacia tile en (-1, 0) |

Combinaciones comunes:

| Constante | Bits | Valor | Descripción |
|-----------|------|-------|-------------|
| `ROAD_X` | SW+NE | 0x0A | Diagonal / (NE↔SW) |
| `ROAD_Y` | NW+SE | 0x05 | Diagonal \ (NW↔SE) |
| `ROAD_ALL` | todos | 0x0F | Cruce 4 vías |

### Conversión de Vecinos a RoadBits

```rust
// Para detectar road_bits desde tiles vecinos:
let mut bits = 0u8;
if vecino_en(x - 1, y) { bits |= 8; } // NE
if vecino_en(x, y + 1) { bits |= 4; } // SE
if vecino_en(x + 1, y) { bits |= 2; } // SW
if vecino_en(x, y - 1) { bits |= 1; } // NW
```

---

---

## Estadísticas de Extracción

El script `scripts/descargar_graficos.sh` extrae actualmente **431 sprites** organizados en:

| Categoría | Cantidad |
|-----------|----------|
| Terreno base | 14 |
| Agua y costas | 15 |
| Carreteras | 23 |
| Vías férreas (rail/mono/maglev) | ~120 |
| Estaciones tren | 18 |
| Paradas bus/truck | 32 |
| Casas urbanas | 50+ |
| Árboles | 42 |
| Industrias | 4 |
| Aeropuertos | 30+ |
| Muelles | 6 |
| Túneles y puentes | 12 |
| Objetos (HQ, transmitter, etc.) | 30+ |
| Vehículos (muestras) | 2 |
| Legacy (compatibilidad) | 4 |

### Cómo añadir más sprites

1. Buscar el ID en `sprites.h` o en este documento
2. Añadir al script: `crop_by_id(<ID>, "<nombre>.png")`
3. Ejecutar `bash scripts/descargar_graficos.sh`
4. Cargar en `main.rs` con `asset_server.load("opengfx/tiles/<nombre>.png")`

### Sprites no extraídos (fuera del NFO base)

- `SPR_BOUGHT_LAND` (4790): requiere otro archivo GRF
- Sprites de extensiones (NewGRF) 
- Animaciones complejas (olas, etc.)

---

*Documento generado a partir de OpenTTD `src/table/sprites.h` (GPLv2) y OpenGFX 8.0.*
