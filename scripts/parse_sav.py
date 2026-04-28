#!/usr/bin/env python3
"""
Convierte un savegame de OpenTTD (.sav) a un archivo binario simple
que puede cargar openttdrs-client sin dependencias externas.

Formato de salida (.ottdmap) v3:
  4 bytes LE  – magic: 0x4D41504F ('MAPO')
  4 bytes LE  – width
  4 bytes LE  – height
  W*H bytes   – tile_type (bits 7-4 = TileType OpenTTD, bits 3-0 = tropic/aux)
  W*H bytes   – height (0-255)
  W*H bytes   – m5 (road bits, TrackBits 0-5, industry gfx bits 0-7, ObjectType en MP_OBJECT)
  W*H bytes   – m1 (industry index, owner, etc.) [v2+]
  W*H bytes   – m6 (bit 2 = bit 8 del gfx industria; StationType en MP_STATION) [v3+]
  W*H*2 bytes – m8 LE (HouseID en MP_HOUSE, 16 bits little-endian) [v3+]

Tipos de tesela OpenTTD (nibble alto de tile_type):
  0  MP_CLEAR       → prado/rough/rocks/fields/desert
  1  MP_RAILWAY     → vía de tren
  2  MP_ROAD        → carretera
  3  MP_HOUSE       → edificio urbano
  4  MP_TREES       → árboles/bosque
  5  MP_STATION     → estación
  6  MP_WATER       → agua
  7  MP_VOID        → borde vacío
  8  MP_INDUSTRY    → industria
  9  MP_TUNNELBRIDGE → túnel/puente
  10 MP_OBJECT      → objeto

Uso:
  python3 scripts/parse_sav.py <archivo.sav> [salida.ottdmap]
"""

import struct
import sys
import zlib
from pathlib import Path


# ---------------------------------------------------------------------------
# Gamma encoding (SlReadSimpleGamma del código C++)
# Formato: 0xxxxxxx / 10xxxxxx xx / 110xxxxx xx xx / 1110xxxx xx xx xx /
#          11110000 xx xx xx xx
# ---------------------------------------------------------------------------

def read_gamma(data: bytes, offset: int) -> tuple[int, int]:
    b = data[offset]; offset += 1
    if not (b & 0x80):
        return b, offset
    b &= ~0x80
    if not (b & 0x40):
        return (b << 8) | data[offset], offset + 1
    b &= ~0x40
    if not (b & 0x20):
        v = (b << 16) | (data[offset] << 8) | data[offset + 1]
        return v, offset + 2
    b &= ~0x20
    if not (b & 0x10):
        v = (b << 24) | (data[offset] << 16) | (data[offset + 1] << 8) | data[offset + 2]
        return v, offset + 3
    b &= ~0x10
    if b & 0x08:
        raise ValueError(f"Gamma no soportado (offset={offset})")
    # Caso 5 bytes: ignorar b, leer 4 bytes big-endian
    v = struct.unpack_from('>I', data, offset)[0]
    return v, offset + 4


# ---------------------------------------------------------------------------
# Saltar cualquier chunk de tipo array (CH_ARRAY, CH_TABLE,
# CH_SPARSE_ARRAY, CH_SPARSE_TABLE).
#
# Todos tienen la misma estructura de stream: secuencia de elementos con
# gamma(N)+N-1 bytes de datos, terminado por gamma=0.
# Para CH_TABLE el primer elemento es el header, pero lo saltamos igual.
# ---------------------------------------------------------------------------

def skip_array(data: bytes, offset: int) -> int:
    while True:
        n, offset = read_gamma(data, offset)
        if n == 0:
            break
        offset += n - 1  # n-1 bytes de datos (SlIterateArray hace --length)
    return offset


# ---------------------------------------------------------------------------
# Parser de chunk CH_RIFF
# ---------------------------------------------------------------------------

def parse_riff(data: bytes, offset: int, m: int) -> tuple[bytes, int]:
    """Devuelve (datos_del_chunk, nuevo_offset)."""
    b2 = data[offset]; offset += 1
    low16 = (data[offset] << 8) | data[offset + 1]; offset += 2
    size = (b2 << 16) | ((m >> 4) << 24) | low16
    chunk_data = data[offset:offset + size]
    return chunk_data, offset + size


# ---------------------------------------------------------------------------
# Parser de chunk CH_TABLE para MAPS (dim_x + dim_y)
# Estructura en stream:
#   gamma(header_bytes+1)  header_bytes de descriptores
#   gamma(data_bytes+1)    data_bytes de datos de campos
#   gamma(0)               fin del array
# ---------------------------------------------------------------------------

def parse_maps_table(data: bytes, offset: int) -> tuple[int, int, int]:
    """Devuelve (dim_x, dim_y, nuevo_offset)."""
    # 1. Elemento header
    n, offset = read_gamma(data, offset)  # = header_bytes + 1
    header_end = offset + n - 1

    # Leer descriptores de campos (solo queremos saber los nombres)
    field_names = []
    while offset < header_end:
        ftype = data[offset]; offset += 1
        if ftype == 0:  # SLE_FILE_END
            break
        name_len, offset = read_gamma(data, offset)
        name = data[offset:offset + name_len].decode('utf-8', errors='replace')
        offset += name_len
        field_names.append((name, ftype))

    offset = header_end  # saltar cualquier padding del header

    # 2. Elemento de datos
    n, offset = read_gamma(data, offset)  # = data_bytes + 1
    if n == 0:
        raise ValueError("MAPS: chunk de datos vacío")
    data_end = offset + n - 1

    # Los campos son SLE_FILE_U32 (4 bytes BE cada uno, en orden declarado)
    values = {}
    for name, ftype in field_names:
        ftype_base = ftype & 0x0F  # SLE_FILE_TYPE_MASK
        if ftype_base == 6:  # SLE_FILE_U32
            values[name] = struct.unpack_from('>I', data, offset)[0]
            offset += 4
        elif ftype_base == 1:  # SLE_FILE_I8
            values[name] = data[offset]; offset += 1
        elif ftype_base == 2:  # SLE_FILE_U8
            values[name] = data[offset]; offset += 1
        elif ftype_base == 3:  # SLE_FILE_I16
            values[name] = struct.unpack_from('>H', data, offset)[0]; offset += 2
        elif ftype_base == 4:  # SLE_FILE_U16
            values[name] = struct.unpack_from('>H', data, offset)[0]; offset += 2
        else:
            break  # no sabemos qué es, saltar

    offset = data_end  # normalizar aunque hayamos leído de más

    # 3. Terminator
    n, offset = read_gamma(data, offset)
    if n != 0:
        # Hay más elementos; saltar
        offset = skip_array(data, offset - len(str(n)))

    dim_x = values.get('dim_x', 0)
    dim_y = values.get('dim_y', 0)
    return dim_x, dim_y, offset


# ---------------------------------------------------------------------------
# Descomprimir el savegame
# ---------------------------------------------------------------------------

MAGIC_OTTZ = b'OTTZ'  # zlib
MAGIC_OTTX = b'OTTX'  # lzma
MAGIC_OTTD = b'OTTD'  # LZO (no soportado aquí)
MAGIC_NONE = b'OTTN'  # sin compresión


def decompress(raw: bytes) -> tuple[bytes, int]:
    """Descomprime el payload y devuelve (datos_descomprimidos, versión_del_savegame)."""
    magic = raw[:4]
    version = struct.unpack('>H', raw[4:6])[0]

    if magic == MAGIC_OTTZ:
        payload = zlib.decompress(raw[8:])
    elif magic == MAGIC_OTTX:
        try:
            import lzma
            payload = lzma.decompress(raw[8:])
        except ImportError:
            raise SystemExit("El módulo lzma no está disponible (instala python3-lzma)")
    elif magic == MAGIC_NONE:
        payload = raw[8:]
    elif magic == MAGIC_OTTD:
        raise SystemExit(
            "Este savegame usa compresión LZO (formato antiguo).\n"
            "Ábrelo en OpenTTD moderno y guárdalo de nuevo para actualizar el formato."
        )
    else:
        raise SystemExit(f"Magic desconocido: {magic!r}")

    return payload, version


# ---------------------------------------------------------------------------
# Parsear chunks y extraer MAPS, MAPT, MAPH, MAP5, MAP2, OBJS
# ---------------------------------------------------------------------------

CH_RIFF          = 0
CH_ARRAY         = 1
CH_SPARSE_ARRAY  = 2
CH_TABLE         = 3
CH_SPARSE_TABLE  = 4


def parse_objs_table(data: bytes, offset: int) -> tuple[dict[int, int], int]:
    """Parsea el chunk OBJS (CH_TABLE) y devuelve {tile_index: object_type}.

    El ObjectType de OpenTTD es:
      0 = OBJECT_TRANSMITTER (antena)
      1 = OBJECT_LIGHTHOUSE  (faro)
    """
    # 1. Header con descriptores de campos
    n, offset = read_gamma(data, offset)
    header = data[offset: offset + n - 1]
    offset += n - 1

    # Parsear nombres y tipos de campos del header
    fields: list[tuple[str, int]] = []
    h = 0
    while h < len(header):
        ftype = header[h]; h += 1
        if ftype == 0:
            break
        name_len, h = read_gamma(header, h)
        fname = header[h: h + name_len].decode('utf-8', errors='replace')
        h += name_len
        fields.append((fname, ftype))

    # 2. Elementos de datos (uno por objeto)
    result: dict[int, int] = {}
    while True:
        n, offset = read_gamma(data, offset)
        if n == 0:
            break
        elem = data[offset: offset + n - 1]
        offset += n - 1

        ep = 0
        vals: dict[str, int] = {}
        for fname, ftype in fields:
            fb = ftype & 0x0F  # SLE_FILE_TYPE bits
            if fb == 6:        # U32
                vals[fname] = struct.unpack_from('>I', elem, ep)[0]; ep += 4
            elif fb == 5:      # I32
                vals[fname] = struct.unpack_from('>i', elem, ep)[0]; ep += 4
            elif fb == 4:      # U16
                vals[fname] = struct.unpack_from('>H', elem, ep)[0]; ep += 2
            elif fb == 3:      # I16
                vals[fname] = struct.unpack_from('>h', elem, ep)[0]; ep += 2
            elif fb in (1, 2): # I8 / U8
                vals[fname] = elem[ep]; ep += 1
            else:
                break          # tipo desconocido, no seguimos

        tile = vals.get('location.tile')
        obj_type = vals.get('type')
        if tile is not None and obj_type is not None:
            result[tile] = obj_type

    return result, offset


def parse_chunks(data: bytes) -> dict[str, bytes | tuple]:
    """Recorre el stream de chunks y devuelve los que necesitamos."""
    chunks: dict = {}
    offset = 0
    total = len(data)

    while offset + 4 < total:
        chunk_id = struct.unpack_from('>I', data, offset)[0]
        offset += 4
        if chunk_id == 0:
            break

        try:
            chunk_name = struct.pack('>I', chunk_id).decode('ascii')
        except Exception:
            chunk_name = '????'

        if offset >= total:
            break

        m = data[offset]; offset += 1
        chunk_type = m & 0x0F

        try:
            if chunk_type == CH_RIFF:
                chunk_data, offset = parse_riff(data, offset, m)
                chunks[chunk_name] = chunk_data

            elif chunk_type == CH_TABLE:
                if chunk_name == 'MAPS':
                    dim_x, dim_y, offset = parse_maps_table(data, offset)
                    chunks['MAPS'] = (dim_x, dim_y)
                elif chunk_name == 'OBJS':
                    obj_types, offset = parse_objs_table(data, offset)
                    chunks['OBJS'] = obj_types
                else:
                    offset = skip_array(data, offset)

            elif chunk_type in (CH_ARRAY, CH_SPARSE_ARRAY, CH_SPARSE_TABLE):
                offset = skip_array(data, offset)

            else:
                print(f"  ⚠ Tipo de chunk desconocido {chunk_type} en '{chunk_name}', deteniendo", file=sys.stderr)
                break

        except Exception as e:
            print(f"  ⚠ Error al parsear chunk '{chunk_name}': {e}", file=sys.stderr)
            break

    return chunks


# ---------------------------------------------------------------------------
# Inferir dimensiones desde el tamaño de MAPT si MAPS no se parseó
# ---------------------------------------------------------------------------

def infer_dimensions(mapt_size: int) -> tuple[int, int] | None:
    """Asume mapa cuadrado (potencia de 2). Devuelve (w, h) o None."""
    import math
    side = int(math.isqrt(mapt_size))
    if side * side == mapt_size and (side & (side - 1)) == 0 and 64 <= side <= 4096:
        return side, side
    # Probar combinaciones no cuadradas (w = 2*h)
    for bits in range(6, 13):
        w = 1 << bits
        for bits2 in range(6, 13):
            h = 1 << bits2
            if w * h == mapt_size:
                return w, h
    return None


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    sav_path = Path(sys.argv[1])
    if not sav_path.exists():
        sys.exit(f"Archivo no encontrado: {sav_path}")

    out_path = Path(sys.argv[2]) if len(sys.argv) > 2 else sav_path.with_suffix('.ottdmap')

    print(f"Leyendo {sav_path} …")
    raw = sav_path.read_bytes()

    data, version = decompress(raw)
    print(f"  Versión del savegame: {version}, tamaño descomprimido: {len(data):,} bytes")

    print("Parseando chunks …")
    chunks = parse_chunks(data)
    found = list(chunks.keys())
    print(f"  Chunks encontrados: {found}")

    # Dimensiones
    if 'MAPS' in chunks and isinstance(chunks['MAPS'], tuple):
        dim_x, dim_y = chunks['MAPS']
    elif 'MAPS' in chunks and isinstance(chunks['MAPS'], (bytes, bytearray)) and len(chunks['MAPS']) >= 8:
        # En saves más viejos MAPS puede venir como CH_RIFF de 8 bytes BE (dim_x, dim_y).
        dim_x, dim_y = struct.unpack_from('>II', chunks['MAPS'], 0)
    elif 'MAPT' in chunks:
        mapt = chunks['MAPT']
        dims = infer_dimensions(len(mapt))
        if dims is None:
            sys.exit(f"No se pueden inferir dimensiones desde MAPT ({len(mapt)} bytes)")
        dim_x, dim_y = dims
        print(f"  ⚠ MAPS no encontrado, inferido: {dim_x}×{dim_y}")
    else:
        sys.exit("Chunk MAPT no encontrado. ¿Es un savegame válido?")

    print(f"  Mapa: {dim_x} × {dim_y} = {dim_x*dim_y:,} teselas")

    # Datos de teselas.
    # Nombres reales de chunks en el savegame (map_sl.cpp de OpenTTD):
    #   MAPT = tile types, MAPH = heights, MAPO = MAP1 (owner),
    #   MAP2 = misc, M3LO/M3HI = MAP3, MAP5 = m5, MAPE = MAP6, MAP7, MAP8
    mapt = chunks.get('MAPT', b'')
    maph = chunks.get('MAPH', b'')
    map5 = chunks.get('MAP5', b'')
    map1 = chunks.get('MAPO', b'')  # MAPO = MAP1 (owner/datos de tesela 1)
    map6 = chunks.get('MAPE', b'')  # MAPE = MAP6 (bit 2 = bit 8 del gfx industria)
    map8 = chunks.get('MAP8', b'')  # MAP8 = HouseID (2 bytes por tesela)
    # OBJS: diccionario {tile_index → ObjectType}  (0=Transmisor, 1=Faro)
    obj_types: dict[int, int] = chunks.get('OBJS', {})  # type: ignore[assignment]

    expected = dim_x * dim_y
    if len(mapt) < expected:
        sys.exit(f"MAPT demasiado corto: {len(mapt)} bytes, esperados {expected}")

    # Padding de los chunks que falten
    if len(maph) < expected:
        maph = maph + b'\x00' * (expected - len(maph))
    if len(map5) < expected:
        map5 = map5 + b'\x00' * (expected - len(map5))
    if len(map1) < expected:
        map1 = map1 + b'\x00' * (expected - len(map1))
    if len(map6) < expected:
        map6 = map6 + b'\x00' * (expected - len(map6))
    if len(map8) < expected * 2:
        map8 = map8 + b'\x00' * (expected * 2 - len(map8))

    # Para tiles MP_OBJECT, sobreescribir m5 con el ObjectType real (de OBJS).
    # En OpenTTD moderno, MAP5 para MP_OBJECT guarda bits altos del ObjectID,
    # no el tipo.  El tipo real se obtiene del array Object a través de OBJS.
    m5_list = bytearray(map5[:expected])
    if obj_types:
        n_fixed = 0
        for i in range(expected):
            if (mapt[i] >> 4) & 0xF == 10:  # MP_OBJECT
                t = obj_types.get(i, 0xFF)
                m5_list[i] = t if t != 0xFF else m5_list[i]
                if t != 0xFF:
                    n_fixed += 1
        print(f"  Objetos con tipo resuelto desde OBJS: {n_fixed}")
    m5_data = bytes(m5_list)

    # Escribir archivo de salida (formato v3: + m6 + m8)
    magic_out = b'MAPO'
    header = struct.pack('<4sII', magic_out, dim_x, dim_y)
    tile_types = mapt[:expected]
    heights    = maph[:expected]
    m1_data    = map1[:expected]
    m6_data    = map6[:expected]
    m8_data    = map8[:expected * 2]

    out_path.write_bytes(header + tile_types + heights + m5_data + m1_data + m6_data + m8_data)
    print(f"✓ Escrito: {out_path}  ({out_path.stat().st_size:,} bytes)")

    # Estadísticas de tipos de tesela
    type_counts: dict[int, int] = {}
    for b in tile_types:
        t = (b >> 4) & 0xF
        type_counts[t] = type_counts.get(t, 0) + 1

    type_names = {
        0: 'Clear', 1: 'Railway', 2: 'Road', 3: 'House', 4: 'Trees',
        5: 'Station', 6: 'Water', 7: 'Void', 8: 'Industry',
        9: 'Tunnelbridge', 10: 'Object',
    }
    print("\nDistribución de tipos de tesela:")
    for t, count in sorted(type_counts.items()):
        name = type_names.get(t, f'Unknown({t})')
        pct = count / expected * 100
        print(f"  {t:2d}  {name:<14}  {count:>8,}  ({pct:.1f}%)")


if __name__ == '__main__':
    main()
