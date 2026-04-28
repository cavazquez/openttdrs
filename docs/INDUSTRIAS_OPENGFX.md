# Sistema de Industrias en OpenTTD

Documentación del sistema de renderizado de industrias basada en el análisis del código fuente de OpenTTD (`table/industry_land.h`, `table/build_industry.h`, `src/industrytype.h`).

## Estructura de datos de tiles de industria

En OpenTTD, los tiles de industria (`MP_INDUSTRY = 8`) usan varios bytes del mapa:

| Byte | Contenido | Descripción |
|------|-----------|-------------|
| `mapt` | nibble alto = 8 | Indica que es `MP_INDUSTRY` |
| `m1` | bits 0-6 = índice | Índice de la industria en el array global |
| `m5` | gfx | Índice del tile dentro del layout de la industria (0-255) |

### El valor `gfx` (m5)

El `gfx` **NO indica qué tipo de industria es**, sino **qué tile específico** del layout es, incluyendo qué sprite usar. Cada gfx apunta a una fila en `_industry_draw_tile_data`.

Estructura del macro `M()`:
```cpp
M( ground_sprite, pal, building_sprite, pal, dx, dy, sx, sy, sz, proc )
```
- `ground_sprite`: sprite del suelo
- `building_sprite`: sprite del edificio (0 = sin edificio)
- `dx, dy`: posición del bounding box en el tile (0-16)
- `sx, sy, sz`: tamaño del bounding box en espacio 3D (sz = altura)
- `proc`: procedimiento especial de dibujo (0 = normal)

Cada gfx tiene 4 entradas en la tabla (stages 0-3). Para industria completada, se usa **stage 3** (índice `gfx * 4 + 3`).

## Tabla completa de gfx → sprite_id (stage 3)

Derivada de `_industry_draw_tile_data` en `table/industry_land.h`. Sprite ID = columna `s2`.

### Coal Mine (gfx 0-6)

| gfx | Sprite ID | Descripción |
|-----|-----------|-------------|
| 0 | 2013 (0x7DD) | Headframe principal |
| 1 | 2015 (0x7DF) | Torre animada |
| 2 | 2018 (0x7E2) | Edificio auxiliar |
| 3 | 2021 (0x7E5) | Edificio pequeño |
| 4 | 0 | Solo suelo |
| 5 | 0 | Solo suelo |
| 6 | 0 | Solo suelo |

Dimensiones exactas del NFO (extraídas de `ogfx1_base.nfo`):
| Sprite | w | h | xrel | yrel |
|--------|---|---|------|------|
| 2013 | 58 | 50 | -16 | -33 |
| 2015 | 46 | 53 | -14 | -38 |
| 2018 | 64 | 39 | -31 | -8 |
| 2021 | 44 | 38 | -13 | -21 |

### Power Station (gfx 7-10)

| gfx | Sprite ID | Descripción |
|-----|-----------|-------------|
| 7 | 2047 (0x7FF) | Chimenea (sz=44, edificio alto) |
| 8 | 2050 (0x802) | Generador |
| 9 | 2053 (0x805) | Transformador |
| 10 | 2054 (0x806) | Edificio principal (proc especial) |

### Sawmill (gfx 11-15)

| gfx | Sprite ID |
|-----|-----------|
| 11 | 2063 (0x80F) |
| 12 | 2066 (0x812) |
| 13 | 2069 (0x815) |
| 14 | 2070 (0x816) |
| 15 | 2071 (0x817) |

### Oil Refinery (gfx 16-23)

| gfx | Sprite ID |
|-----|-----------|
| 16 | 2075 (0x81B) |
| 17 | 2076 (0x81C) |
| 18 | 2080 (0x820) |
| 19 | 2083 (0x823) |
| 20 | 2086 (0x826) |
| 21 | 2089 (0x829) |
| 22 | 2092 (0x82C) |
| 23 | 2095 (0x82F) |

### Forest (gfx 24-28)

| gfx | Sprite ID | Descripción |
|-----|-----------|-------------|
| 24 | 0 | Suelo animado (sin overlay estático) |
| 25 | 2099 (0x833) | Cluster de árboles 1 |
| 26 | 2100 (0x834) | Cluster de árboles 2 |
| 27 | 2101 (0x835) | Cluster de árboles 3 |
| 28 | 2102 (0x836) | Cluster de árboles 4 |

### Printing Works (gfx 29-32)

| gfx | Sprite ID |
|-----|-----------|
| 29 | 2174 (0x87E) |
| 30 | 2178 (0x882) |
| 31 | 2177 (0x881) |
| 32 | 2174 (0x87E) |

### Oil Rig (gfx 33-38)

| gfx | Sprite ID |
|-----|-----------|
| 33 | 2108 (0x83C) |
| 34 | 2109 (0x83D) |
| 35 | 2111 (0x83F) |
| 36 | 2113 (0x841) |
| 37 | 2115 (0x843) |
| 38 | 2117 (0x845) |

### Steel Mill (gfx 39-42)

| gfx | Sprite ID |
|-----|-----------|
| 39 | 2150 (0x866) |
| 40 | 2151 (0x867) |
| 41 | 2152 (0x868) |
| 42 | 0 (solo suelo) |

### Factory (gfx 43-46)

| gfx | Sprite ID |
|-----|-----------|
| 43 | 2169 (0x879) |
| 44 | 2170 (0x87A) |
| 45 | 2171 (0x87B) |
| 46 | 2172 (0x87C) |

### Oil Wells (gfx 47-51)

| gfx | Sprite ID |
|-----|-----------|
| 47 | 2028 (0x7EC) |
| 48 | 2030 (0x7EE) |
| 49 | 2033 (0x7F1) |
| 50 | 2036 (0x7F4) |
| 51 | 2039 (0x7F7) |

### Farm (gfx 52-57)

| gfx | Sprite ID | Descripción |
|-----|-----------|-------------|
| 52 | 2119 (0x847) | Edificio 1 |
| 53 | 2121 (0x849) | Edificio 2 |
| 54 | 2123 (0x84B) | Edificio 3 |
| 55 | 0 | Campo (sin edificio) |
| 56 | 2126 (0x84E) | Edificio 4 |
| 57 | 2128 (0x850) | Granero |

### Bank Templado (gfx 58-59)

| gfx | Sprite ID |
|-----|-----------|
| 58 | 2180 (0x884) |
| 59 | 2181 (0x885) |

## Implementación en openttdrs

### Formato `.ottdmap` v2

```
4 bytes LE  – magic: 'MAPO'
4 bytes LE  – width
4 bytes LE  – height
W*H bytes   – tile_type (mapt)
W*H bytes   – height
W*H bytes   – m5 (gfx para industrias)
W*H bytes   – m1 (índice de industria) [v2]
```

### Struct `IndustryGfxSprite` (`sprites.rs`)

```rust
pub struct IndustryGfxSprite {
    pub sprite_id: u32,  // 0 = solo suelo
    pub w: f32,
    pub h: f32,
    pub xrel: f32,       // offset horizontal desde vértice superior del rombo
    pub yrel: f32,       // offset vertical (negativo = arriba)
}
```

### Función `industry_sprite_for_gfx` (`sprites.rs`)

```rust
pub fn industry_sprite_for_gfx(gfx: u8) -> Option<&'static IndustryGfxSprite> {
    let entry = INDUSTRY_GFX_DATA.get(usize::from(gfx))?;
    if entry.sprite_id != 0 { Some(entry) } else { None }
}
```

### Carga dinámica de sprites (`main.rs`)

La carga es totalmente dinámica: itera `INDUSTRY_GFX_DATA` y carga todos los
`sprite_id` únicos como `opengfx/tiles/industry_{sprite_id}.png`.

Agregar soporte a una nueva industria = solo actualizar `INDUSTRY_GFX_DATA`.

### Convención de nombres de archivos

Los sprites se extraen como `industry_{sprite_id}.png`:
- `industry_2013.png` → Coal Mine headframe
- `industry_2047.png` → Power Station chimney
- etc.

## Calibración de offsets (xrel, yrel)

Al ejecutar `scripts/descargar_graficos.sh`, el script imprime para cada sprite:
```
  industry_2013.png (58×50 xrel=-16 yrel=-33) ← sprite 2013
```

Usar estos valores para actualizar `INDUSTRY_GFX_DATA` y obtener posicionamiento exacto.

Los valores actuales de la tabla usan `w=64, h=48, xrel=-32, yrel=-32` como default
para todas las industrias excepto Coal Mine (que tiene valores exactos del NFO).

## Cómo identificar el tipo de industria

1. **Método directo**: Leer `m1` (bits 0-6) para obtener el índice de industria, luego consultar el chunk `INDY` del savegame.

2. **Método por gfx**: Inferir del rango de `gfx` (ver tabla arriba). No es 100% confiable con NewGRFs.

## Referencias

- `src/table/industry_land.h` — Tabla `_industry_draw_tile_data` con todos los sprites
- `src/table/build_industry.h` — Layouts y especificaciones por industria
- `src/industrytype.h` — Enum `IndustryType` con IDs de industria
- `src/industry_map.h` — Funciones de acceso a tiles de industria
