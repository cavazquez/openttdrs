# Sistema de Industrias en OpenTTD

Documentación del sistema de renderizado de industrias basada en el análisis del código fuente de OpenTTD.

## Estructura de datos de tiles de industria

En OpenTTD, los tiles de industria (`MP_INDUSTRY = 8`) usan varios bytes del mapa:

| Byte | Contenido | Descripción |
|------|-----------|-------------|
| `mapt` | nibble alto = 8 | Indica que es `MP_INDUSTRY` |
| `m1` | bits 0-6 = índice | Índice de la industria en el array global |
| `m5` | gfx | Índice del tile dentro del layout de la industria |
| `m6` | bit 2 | Bit alto del gfx (para gfx > 255) |

### El valor `gfx` (m5)

El `gfx` **NO indica qué tipo de industria es**, sino **qué parte del layout** es ese tile dentro de la industria.

Por ejemplo, una Coal Mine ocupa ~6 tiles en un patrón 2×3:
```
Layout ejemplo:
  [gfx 5] [gfx 0]
  [gfx 4] [gfx 1]
  [gfx 6] [gfx 2/3]
```

Cada `gfx` corresponde a un sprite diferente (torre, edificio auxiliar, entrada, o solo suelo).

## Mapeo gfx → sprite

La tabla `_industry_draw_tile_data` en `table/industry_land.h` define el mapeo:

```
índice = gfx * 4 + construction_stage
```

Donde `construction_stage` es 0-3 (etapas de construcción). Para industria completada, stage = 3.

### Coal Mine (IT_COAL_MINE = 0)

| gfx | Sprite ID | Descripción |
|-----|-----------|-------------|
| 0 | 2013 (0x7dd) | Headframe principal (torre de extracción) |
| 1 | 2015 (0x7df) | Torre animada |
| 2 | 2018 (0x7e2) | Edificio auxiliar |
| 3 | 2021 (0x7e5) | Edificio pequeño |
| 4 | 0 | Solo suelo (sin edificio) |
| 5 | 0 | Solo suelo |
| 6 | 0 | Solo suelo |

### Sprites de Coal Mine disponibles en OpenGFX

Extraídos de `ogfx1_base.nfo`:

| Archivo | Sprite ID | Dimensiones | xrel | yrel | Descripción |
|---------|-----------|-------------|------|------|-------------|
| `industry_coalmine_hq.png` | 2013 | 58×50 | -16 | -33 | Headframe principal |
| `industry_coalmine_tower.png` | 2028 | 46×53 | -14 | -38 | Torre alternativa |
| `industry_coalmine_entry.png` | 2011 | 36×25 | -17 | -7 | Entrada/componente |

## Layouts de industrias

Definidos en `table/build_industry.h`:

```cpp
// Coal Mine tiene 4 variantes de layout
static const IndustryTileLayout _tile_table_coal_mine_0 {
    MK(1, 1, 0),  // x=1, y=1, gfx=0 (headframe)
    MK(1, 2, 2),  // x=1, y=2, gfx=2 (edificio aux)
    MK(0, 0, 5),  // x=0, y=0, gfx=5 (suelo)
    MK(0, 1, 1),  // x=0, y=1, gfx=1 (torre)
    MK(0, 2, 4),  // x=0, y=2, gfx=4 (suelo)
    MK(2, 2, 3),  // x=2, y=2, gfx=3 (edificio peq)
};
```

El macro `MK(x, y, gfx)` define la posición relativa y el gfx de cada tile.

## Tipos de industria (IndustryTypes)

Enum en `table/build_industry.h`:

| ID | Nombre | gfx range |
|----|--------|-----------|
| 0 | IT_COAL_MINE | 0-6 |
| 1 | IT_POWER_STATION | 7-14 |
| 2 | IT_SAWMILL | 15-22 |
| 3 | IT_FOREST | 23+ |
| ... | ... | ... |

## Cómo identificar el tipo de industria

Para saber qué industria es un tile:

1. **Método directo**: Leer `m1` (bits 0-6) para obtener el índice de industria, luego consultar el chunk `INDY` del savegame para obtener el tipo.

2. **Método aproximado**: Inferir del rango de `gfx`:
   - gfx 0-6: Probablemente Coal Mine
   - gfx 7-14: Probablemente Power Station
   - etc.

   Pero esto no es 100% confiable porque NewGRFs pueden cambiar los rangos.

## Implementación actual en openttdrs

### Archivo `.ottdmap` formato v2

```
4 bytes LE  – magic: 'MAPO'
4 bytes LE  – width
4 bytes LE  – height
W*H bytes   – tile_type (mapt)
W*H bytes   – height
W*H bytes   – m5 (gfx para industrias)
W*H bytes   – m1 (índice de industria) [v2]
```

### Función `industry_sprite_for_gfx`

En `sprites.rs`:

```rust
pub fn industry_sprite_for_gfx(gfx: u8) -> Option<(u32, f32, f32, f32, f32)> {
    // Solo Coal Mine implementada (gfx 0-6)
    if gfx < 7 {
        let entry = INDUSTRY_GFX_SPRITES[gfx as usize];
        if entry.0 != 0 {
            return Some(entry);
        }
    }
    // Fallback para otros gfx
    ...
}
```

## Trabajo futuro

1. **Extraer más sprites**: Power Station, Sawmill, Oil Refinery, etc.

2. **Parsear chunk INDY**: Para obtener el tipo exacto de cada industria.

3. **Tabla completa de gfx→sprite**: Cubrir todas las industrias del juego base.

4. **Soporte de animaciones**: Algunas industrias tienen tiles animados (chimeneas, torres rotando).

## Referencias

- `src/industry_map.h` - Funciones de acceso a tiles de industria
- `src/table/industry_land.h` - Tabla `_industry_draw_tile_data`
- `src/table/build_industry.h` - Layouts y especificaciones
- `src/industrytype.h` - Enum `IndustryType`
