#!/usr/bin/env python3
"""
Convierte un savegame de OpenTTD (.sav) a un archivo binario simple
que puede cargar openttdrs-client sin dependencias externas.

Especificación detallada del binario: docs/OTTDMAP_FORMAT.md (raíz del repo).

Formato de salida (.ottdmap) versionado:
  4 bytes LE  – magic: 0x4D415031 ('MAP1')
  4 bytes LE  – width
  4 bytes LE  – height
  2 bytes LE  – format_version (actual: 1)
  2 bytes LE  – flags (actualmente reservado)
  W*H bytes   – tile_type (bits 7-4 = TileType OpenTTD, bits 3-0 = tropic/aux)
  W*H bytes   – height (0-255)
  W*H bytes   – m1 (industry index, owner, etc.)
  W*H bytes   – m2 bajo (MAP2 LE byte 0; en save OpenTTD `m2()` es u16)
  W*H bytes   – m2_hi (byte alto MAP2; reserva PBS / bits altos de `m2()`)
  W*H bytes   – m3 (M3LO)
  W*H bytes   – m3hi (chunk M3HI = **`m4()`** en `map_sl.cpp`, p.ej. estados de señal)
  W*H bytes   – m5 (road bits, TrackBits 0-5, industry gfx bits 0-7, ObjectType en MP_OBJECT)
  W*H bytes   – m6 (bit 2 = bit 8 del gfx industria; StationType en MP_STATION)
  W*H bytes   – m7 (MAP7)
  W*H*2 bytes – m8 LE (HouseID en MP_HOUSE; en MP_ROAD bits altos incluyen RoadType tram)
  Footers opcionales (magic ASCII + u32 LE length + payload):
    INDP  – industrias: count × (u16 industry_index, u8 industry_type)
    STNN  – blob crudo del chunk STNN (CH_TABLE o CH_ARRAY según versión del save)
    TNBP  – blob de chunk TNBP / TBUS / TUNN si existe (CH_ARRAY o CH_TABLE; p. ej. JGR `TUNN` es tabla Sl)
    STXY  – teselas MP_STATION: u32 count + count × (u16 x, u16 y) en coordenadas de mapa
            (derivado del plano MAPT; no sustituye decodificar STNN para waypoints en vía)

  NewGRF: exportar MAP7/m8/m3hi no sustituye GRFs ni lógica de specs; ver docs.

  En saves OpenTTD con versión < 348 el HouseID en disco está en M3HI/M3LO;
  parse_sav.py lo copia a m8 como hace afterload.cpp al cargar.

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

from __future__ import annotations

import struct
import sys
import zlib
from pathlib import Path


# ---------------------------------------------------------------------------
# Gamma encoding (SlReadSimpleGamma del código C++)
# ---------------------------------------------------------------------------

def read_gamma(data: bytes, offset: int) -> tuple[int, int]:
    b = data[offset]
    offset += 1
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
    v = struct.unpack_from(">I", data, offset)[0]
    return v, offset + 4


def skip_array(data: bytes, offset: int) -> int:
    while True:
        n, offset = read_gamma(data, offset)
        if n == 0:
            break
        offset += n - 1
    return offset


def slurp_array_payload(data: bytes, offset: int) -> tuple[bytes, int]:
    """Consume un CH_ARRAY / CH_TABLE / sparse array y devuelve (bytes crudos, nuevo_offset)."""
    start = offset
    end = skip_array(data, offset)
    return data[start:end], end


def parse_riff(data: bytes, offset: int, m: int) -> tuple[bytes, int]:
    b2 = data[offset]
    offset += 1
    low16 = (data[offset] << 8) | data[offset + 1]
    offset += 2
    size = (b2 << 16) | ((m >> 4) << 24) | low16
    chunk_data = data[offset : offset + size]
    return chunk_data, offset + size


def parse_maps_table(data: bytes, offset: int) -> tuple[int, int, int]:
    n, offset = read_gamma(data, offset)
    header_end = offset + n - 1

    field_names = []
    while offset < header_end:
        ftype = data[offset]
        offset += 1
        if ftype == 0:
            break
        name_len, offset = read_gamma(data, offset)
        name = data[offset : offset + name_len].decode("utf-8", errors="replace")
        offset += name_len
        field_names.append((name, ftype))

    offset = header_end

    n, offset = read_gamma(data, offset)
    if n == 0:
        raise ValueError("MAPS: chunk de datos vacío")
    data_end = offset + n - 1

    values: dict = {}
    for name, ftype in field_names:
        ftype_base = ftype & 0x0F
        if ftype_base == 6:
            values[name] = struct.unpack_from(">I", data, offset)[0]
            offset += 4
        elif ftype_base == 1:
            values[name] = data[offset]
            offset += 1
        elif ftype_base == 2:
            values[name] = data[offset]
            offset += 1
        elif ftype_base == 3:
            values[name] = struct.unpack_from(">H", data, offset)[0]
            offset += 2
        elif ftype_base == 4:
            values[name] = struct.unpack_from(">H", data, offset)[0]
            offset += 2
        else:
            break

    offset = data_end
    offset = skip_array(data, offset)

    dim_x = values.get("dim_x", 0)
    dim_y = values.get("dim_y", 0)
    return dim_x, dim_y, offset


MAGIC_OTTZ = b"OTTZ"
MAGIC_OTTX = b"OTTX"
MAGIC_OTTD = b"OTTD"
MAGIC_NONE = b"OTTN"

SLV_INCREASE_HOUSE_LIMIT = 348
MP_HOUSE = 3
MP_WATER = 6

# INDY (CH_ARRAY): offset del byte ``Industry.type`` dentro del objeto Industry
# para saves recientes (p. ej. versión 211 del fixture); ver industry_sl.cpp.
INDY_TYPE_BYTE_OFFSET = 9

CH_RIFF = 0
CH_ARRAY = 1
CH_SPARSE_ARRAY = 2
CH_TABLE = 3
CH_SPARSE_TABLE = 4
def build_m8_le_for_save(
    version: int,
    mapt: bytes,
    map8_raw: bytes,
    m3lo_raw: bytes,
    m3hi_raw: bytes,
    expected: int,
) -> bytes:
    buf = bytearray(map8_raw[: expected * 2])
    if len(buf) < expected * 2:
        buf.extend(b"\x00" * (expected * 2 - len(buf)))
    m3lo = m3lo_raw[:expected].ljust(expected, b"\x00")
    m3hi = m3hi_raw[:expected].ljust(expected, b"\x00")
    if version < SLV_INCREASE_HOUSE_LIMIT:
        for i in range(expected):
            if ((mapt[i] >> 4) & 0xF) != MP_HOUSE:
                continue
            hid = m3hi[i] | (((m3lo[i] >> 6) & 1) << 8)
            struct.pack_into("<H", buf, i * 2, hid & 0xFFFF)
    return bytes(buf)


def dimensions_from_chunks(chunks: dict) -> tuple[int, int]:
    if "MAPS" in chunks and isinstance(chunks["MAPS"], tuple):
        dim_x, dim_y = chunks["MAPS"]
        return dim_x, dim_y
    if "MAPS" in chunks and isinstance(chunks["MAPS"], (bytes, bytearray)) and len(chunks["MAPS"]) >= 8:
        dim_x, dim_y = struct.unpack_from(">II", chunks["MAPS"], 0)
        return dim_x, dim_y
    if "MAPT" in chunks:
        mapt = chunks["MAPT"]
        dims = infer_dimensions(len(mapt))
        if dims is None:
            raise ValueError(f"No se pueden inferir dimensiones desde MAPT ({len(mapt)} bytes)")
        return dims
    raise ValueError("Chunk MAPT no encontrado")


def parse_indy_ch_array(data: bytes, offset: int) -> tuple[list[tuple[int, int]], int]:
    """Parsea INDY como CH_ARRAY: devuelve [(industry_index, type_u8), ...] y nuevo offset.

    El índice sigue el orden de SlIterateArray (0..n-1) en CH_ARRAY de OpenTTD.
    ``type`` se lee en byte fijo ``INDY_TYPE_BYTE_OFFSET`` (válido para saves ~200+).
    """
    out: list[tuple[int, int]] = []
    idx = 0
    while True:
        n, offset = read_gamma(data, offset)
        if n == 0:
            break
        body = data[offset : offset + n - 1]
        offset += n - 1
        if len(body) > INDY_TYPE_BYTE_OFFSET:
            out.append((idx, body[INDY_TYPE_BYTE_OFFSET]))
        idx += 1
    return out, offset


def build_indp_footer(pairs: list[tuple[int, int]]) -> bytes:
    parts = [b"INDP", struct.pack("<I", len(pairs))]
    for i, t in pairs:
        parts.append(struct.pack("<HB", i & 0xFFFF, t & 0xFF))
    return b"".join(parts)


def build_stxy_footer(tile_types: bytes, dim_x: int, dim_y: int) -> bytes:
    """Lista teselas con nibble alto MAPT = MP_STATION (5), orden i = y*dim_x + x."""
    expected = dim_x * dim_y
    coords: list[tuple[int, int]] = []
    for i in range(min(len(tile_types), expected)):
        if ((tile_types[i] >> 4) & 0xF) == 5:
            coords.append((i % dim_x, i // dim_x))
    parts = [b"STXY", struct.pack("<I", len(coords))]
    for x, y in coords:
        parts.append(struct.pack("<HH", x & 0xFFFF, y & 0xFFFF))
    return b"".join(parts)


def parse_objs_table(data: bytes, offset: int) -> tuple[dict[int, int], int]:
    n, offset = read_gamma(data, offset)
    header = data[offset : offset + n - 1]
    offset += n - 1

    fields: list[tuple[str, int]] = []
    h = 0
    while h < len(header):
        ftype = header[h]
        h += 1
        if ftype == 0:
            break
        name_len, h = read_gamma(header, h)
        fname = header[h : h + name_len].decode("utf-8", errors="replace")
        h += name_len
        fields.append((fname, ftype))

    result: dict[int, int] = {}
    while True:
        n, offset = read_gamma(data, offset)
        if n == 0:
            break
        elem = data[offset : offset + n - 1]
        offset += n - 1

        ep = 0
        vals: dict[str, int] = {}
        for fname, ftype in fields:
            fb = ftype & 0x0F
            if fb == 6:
                vals[fname] = struct.unpack_from(">I", elem, ep)[0]
                ep += 4
            elif fb == 5:
                vals[fname] = struct.unpack_from(">i", elem, ep)[0]
                ep += 4
            elif fb == 4:
                vals[fname] = struct.unpack_from(">H", elem, ep)[0]
                ep += 2
            elif fb == 3:
                vals[fname] = struct.unpack_from(">h", elem, ep)[0]
                ep += 2
            elif fb in (1, 2):
                vals[fname] = elem[ep]
                ep += 1
            else:
                break

        tile = vals.get("location.tile")
        obj_type = vals.get("type")
        if tile is not None and obj_type is not None:
            result[tile] = obj_type

    return result, offset


def analyze_save(raw: bytes) -> dict:
    data, version = decompress(raw)
    chunks = parse_chunks(data)
    dim_x, dim_y = dimensions_from_chunks(chunks)
    expected = dim_x * dim_y
    mapt = chunks.get("MAPT", b"")
    if len(mapt) < expected:
        raise ValueError(f"MAPT demasiado corto: {len(mapt)} bytes, esperados {expected}")
    map8 = chunks.get("MAP8", b"")
    m3lo = chunks.get("M3LO", b"")
    m3hi = chunks.get("M3HI", b"")
    if len(map8) < expected * 2:
        map8 = map8 + b"\x00" * (expected * 2 - len(map8))
    if len(m3lo) < expected:
        m3lo = m3lo + b"\x00" * (expected - len(m3lo))
    if len(m3hi) < expected:
        m3hi = m3hi + b"\x00" * (expected - len(m3hi))

    m8_data = build_m8_le_for_save(version, mapt, map8, m3lo, m3hi, expected)

    type_counts: dict[str, int] = {}
    for b in mapt[:expected]:
        t = (b >> 4) & 0xF
        key = str(t)
        type_counts[key] = type_counts.get(key, 0) + 1

    hist: dict[str, int] = {}
    n_house = 0
    for i in range(expected):
        if ((mapt[i] >> 4) & 0xF) != MP_HOUSE:
            continue
        n_house += 1
        hid = struct.unpack_from("<H", m8_data, i * 2)[0]
        key = str(hid)
        hist[key] = hist.get(key, 0) + 1

    map5 = chunks.get("MAP5", b"")
    map5 = map5[:expected].ljust(expected, b"\x00")
    water = water_tile_type_counts(mapt[:expected], map5)
    road_normal = 0
    road_normal_tram_bits = 0
    for i in range(expected):
        if ((mapt[i] >> 4) & 0xF) != 2:
            continue
        if (map5[i] >> 6) & 0x3 != 0:
            continue
        road_normal += 1
        if m3lo[i] & 0x0F:
            road_normal_tram_bits += 1

    indp_pairs: list[tuple[int, int]] = []
    if "INDY" in chunks and isinstance(chunks["INDY"], list):
        indp_pairs = list(chunks["INDY"])  # type: ignore[assignment]

    return {
        "save_version": version,
        "dimensions": [dim_x, dim_y],
        "tile_type_counts": {k: type_counts[k] for k in sorted(type_counts, key=int)},
        "house": {
            "tiles": n_house,
            "unique_m8": len(hist),
            "m8_histogram": {k: hist[k] for k in sorted(hist, key=int)},
        },
        "road": {
            "normal_tiles": road_normal,
            "normal_with_tram_track_bits": road_normal_tram_bits,
        },
        "water": water,
        "industry_pairs": len(indp_pairs),
    }


def water_tile_type_counts(mapt: bytes, map5: bytes) -> dict[str, int]:
    """Histograma de ``WaterTileType`` (bits 4–7 de MAP5) en teselas ``MP_WATER``."""
    n = min(len(mapt), len(map5))
    counts: dict[str, int] = {"tiles": 0, "clear": 0, "coast": 0, "other": 0}
    for i in range(n):
        if (mapt[i] >> 4) & 0xF != MP_WATER:
            continue
        counts["tiles"] += 1
        wtt = (map5[i] >> 4) & 0x0F
        if wtt == 0:
            counts["clear"] += 1
        elif wtt == 1:
            counts["coast"] += 1
        else:
            counts["other"] += 1
    return counts


def export_ottdmap_from_chunks(chunks: dict, version: int) -> bytes:
    """Construye el binario ``.ottdmap`` (MAP1 v1) a partir de chunks ya parseados."""
    dim_x, dim_y = dimensions_from_chunks(chunks)
    expected = dim_x * dim_y

    mapt = chunks.get("MAPT", b"")
    if len(mapt) < expected:
        raise ValueError(f"MAPT demasiado corto: {len(mapt)} bytes, esperados {expected}")

    maph = chunks.get("MAPH", b"")
    map5 = chunks.get("MAP5", b"")
    map1 = chunks.get("MAPO", b"")
    map6 = chunks.get("MAPE", b"")
    map8 = chunks.get("MAP8", b"")
    m3lo = chunks.get("M3LO", b"")
    m3hi = chunks.get("M3HI", b"")
    map2 = chunks.get("MAP2", b"")
    map7 = chunks.get("MAP7", b"")
    obj_types: dict[int, int] = chunks.get("OBJS", {})  # type: ignore[assignment]

    if len(maph) < expected:
        maph = maph + b"\x00" * (expected - len(maph))
    if len(map5) < expected:
        map5 = map5 + b"\x00" * (expected - len(map5))
    if len(map1) < expected:
        map1 = map1 + b"\x00" * (expected - len(map1))
    if len(map6) < expected:
        map6 = map6 + b"\x00" * (expected - len(map6))
    if len(map8) < expected * 2:
        map8 = map8 + b"\x00" * (expected * 2 - len(map8))
    if len(m3lo) < expected:
        m3lo = m3lo + b"\x00" * (expected - len(m3lo))
    if len(m3hi) < expected:
        m3hi = m3hi + b"\x00" * (expected - len(m3hi))
    if len(map2) < expected:
        map2 = map2 + b"\x00" * (expected - len(map2))
    if len(map7) < expected:
        map7 = map7 + b"\x00" * (expected - len(map7))

    m5_list = bytearray(map5[:expected])
    if obj_types:
        for i in range(expected):
            if (mapt[i] >> 4) & 0xF == 10:
                t = obj_types.get(i, 0xFF)
                m5_list[i] = t if t != 0xFF else m5_list[i]
    m5_data = bytes(m5_list)

    m8_data = build_m8_le_for_save(version, mapt[:expected], map8, m3lo, m3hi, expected)
    m3_export = m3lo[:expected]
    if len(map2) >= 2 * expected:
        m2_lo = bytes(map2[i * 2] for i in range(expected))
        m2_hi_plane = bytes(map2[i * 2 + 1] for i in range(expected))
    else:
        m2_lo = (map2[:expected] if len(map2) >= expected else map2 + b"\x00" * expected)[:expected]
        m2_hi_plane = b"\x00" * expected
    m7_export = map7[:expected]
    m3hi_export = m3hi[:expected]

    magic_out = b"MAP1"
    format_version = 1
    flags = 1 << 0  # HAS_M2_HI
    header = struct.pack("<4sIIHH", magic_out, dim_x, dim_y, format_version, flags)
    tile_types = mapt[:expected]
    heights = maph[:expected]
    m1_data = map1[:expected]
    m6_data = map6[:expected]

    body = (
        header
        + tile_types
        + heights
        + m1_data
        + m2_lo
        + m2_hi_plane
        + m3_export
        + m3hi_export
        + m5_data
        + m6_data
        + m7_export
        + m8_data
    )

    indp_pairs: list[tuple[int, int]] = []
    if "INDY" in chunks and isinstance(chunks["INDY"], list):
        indp_pairs = chunks["INDY"]  # type: ignore[assignment]
    body += build_indp_footer(indp_pairs)

    stnn_blob = chunks.get("STNN", b"")
    if isinstance(stnn_blob, (bytes, bytearray)) and stnn_blob:
        body += b"STNN" + struct.pack("<I", len(stnn_blob)) + bytes(stnn_blob)

    tnbp_blob = b""
    for _k in ("TNBP", "TBUS", "TUNN"):
        b = chunks.get(_k, b"")
        if isinstance(b, (bytes, bytearray)) and b:
            tnbp_blob = bytes(b)
            break
    if tnbp_blob:
        body += b"TNBP" + struct.pack("<I", len(tnbp_blob)) + tnbp_blob

    body += build_stxy_footer(tile_types, dim_x, dim_y)
    return body


def ottdmap_dense_m5_plane(data: bytes) -> tuple[int, int, bytes]:
    """Devuelve ``(width, height, m5)`` del bloque denso MAP1 v1."""
    if len(data) < 16 or data[0:4] != b"MAP1":
        raise ValueError("cabecera MAP1 inválida")
    dim_x, dim_y = struct.unpack_from("<II", data, 4)
    fmt_ver, flags = struct.unpack_from("<HH", data, 12)
    if fmt_ver != 1:
        raise ValueError(f"format_version {fmt_ver} no soportado")
    n = dim_x * dim_y
    base = 16
    dense = 12 * n if flags & 1 else 11 * n
    if len(data) < base + dense:
        raise ValueError("bloque denso incompleto")
    off = base + 7 * n  # MAPT, MAPH, m1, m2, m2_hi, m3, m3hi
    return dim_x, dim_y, data[off : off + n]


def decompress(raw: bytes) -> tuple[bytes, int]:
    magic = raw[:4]
    version = struct.unpack(">H", raw[4:6])[0]

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
        payload = _decompress_lzo(raw[8:])
    else:
        raise SystemExit(f"Magic desconocido: {magic!r}")

    return payload, version


def _decompress_lzo(payload: bytes) -> bytes:
    try:
        import lzo
    except ImportError:
        raise SystemExit(
            "Este savegame usa compresión LZO (OTTD).\n"
            "Instalá:  pip install python-lzo\n"
            "O abrí el archivo en OpenTTD moderno y guardalo como OTTZ."
        ) from None
    return lzo.decompress(payload)


def parse_chunks(
    data: bytes,
    chunk_type_trace: list[tuple[str, int]] | None = None,
) -> dict[str, bytes | tuple | list | dict]:
    chunks: dict = {}
    offset = 0
    total = len(data)

    while offset + 4 < total:
        chunk_id = struct.unpack_from(">I", data, offset)[0]
        offset += 4
        if chunk_id == 0:
            break

        raw_name = struct.pack(">I", chunk_id)
        chunk_name = raw_name.decode("ascii", errors="replace")
        if len(chunk_name) != 4 or not all(32 <= ord(c) < 127 for c in chunk_name):
            chunk_name = "????"

        if offset >= total:
            break

        m = data[offset]
        offset += 1
        chunk_type = m & 0x0F
        if chunk_type_trace is not None:
            chunk_type_trace.append((chunk_name, chunk_type))

        try:
            if chunk_type == CH_RIFF:
                chunk_data, offset = parse_riff(data, offset, m)
                chunks[chunk_name] = chunk_data

            elif chunk_type == CH_TABLE:
                if chunk_name == "MAPS":
                    dim_x, dim_y, offset = parse_maps_table(data, offset)
                    chunks["MAPS"] = (dim_x, dim_y)
                elif chunk_name == "OBJS":
                    obj_types, offset = parse_objs_table(data, offset)
                    chunks["OBJS"] = obj_types
                elif chunk_name == "STNN":
                    blob, offset = slurp_array_payload(data, offset)
                    chunks["STNN"] = blob
                elif chunk_name in ("TNBP", "TBUS", "TUNN"):
                    # JGRPP guarda túneles en `TUNN` como CH_TABLE (mismo envoltorio gamma que STNN).
                    blob, offset = slurp_array_payload(data, offset)
                    chunks[chunk_name] = blob
                else:
                    offset = skip_array(data, offset)

            elif chunk_type in (CH_ARRAY, CH_SPARSE_ARRAY, CH_SPARSE_TABLE):
                if chunk_name == "INDY" and chunk_type == CH_ARRAY:
                    pairs, offset = parse_indy_ch_array(data, offset)
                    chunks["INDY"] = pairs
                elif chunk_name == "STNN":
                    blob, offset = slurp_array_payload(data, offset)
                    chunks["STNN"] = blob
                elif chunk_name in ("TNBP", "TBUS", "TUNN"):
                    blob, offset = slurp_array_payload(data, offset)
                    chunks[chunk_name] = blob
                else:
                    offset = skip_array(data, offset)

            elif chunk_type == 5:
                # CH_READONLY: sin formato de payload conocido en este parser.
                print(
                    f"  ⚠ CH_READONLY (5) en '{chunk_name}' offset={offset - 5}; se detiene el parseo",
                    file=sys.stderr,
                )
                break

            else:
                print(
                    f"  ⚠ Tipo de chunk desconocido {chunk_type} en '{chunk_name}' "
                    f"(offset={offset - 5}), se omite el resto del stream",
                    file=sys.stderr,
                )
                break

        except Exception as e:
            print(f"  ⚠ Error al parsear chunk '{chunk_name}': {e}", file=sys.stderr)
            break

    return chunks


def infer_dimensions(mapt_size: int) -> tuple[int, int] | None:
    import math

    side = int(math.isqrt(mapt_size))
    if side * side == mapt_size and (side & (side - 1)) == 0 and 64 <= side <= 4096:
        return side, side
    for bits in range(6, 13):
        w = 1 << bits
        for bits2 in range(6, 13):
            h = 1 << bits2
            if w * h == mapt_size:
                return w, h
    return None


def main() -> None:
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    sav_path = Path(sys.argv[1])
    if not sav_path.exists():
        sys.exit(f"Archivo no encontrado: {sav_path}")

    out_path = Path(sys.argv[2]) if len(sys.argv) > 2 else sav_path.with_suffix(".ottdmap")

    print(f"Leyendo {sav_path} …")
    raw = sav_path.read_bytes()

    data, version = decompress(raw)
    print(f"  Versión del savegame: {version}, tamaño descomprimido: {len(data):,} bytes")

    print("Parseando chunks …")
    chunks = parse_chunks(data)
    found = list(chunks.keys())
    print(f"  Chunks encontrados: {found}")

    if "MAPS" not in chunks and "MAPT" in chunks:
        mapt0 = chunks["MAPT"]
        inferred = infer_dimensions(len(mapt0))
        if inferred is not None:
            print(f"  ⚠ MAPS no encontrado, inferido: {inferred[0]}×{inferred[1]}")
    try:
        dim_x, dim_y = dimensions_from_chunks(chunks)
    except ValueError as e:
        msg = str(e) if str(e) else "Chunk MAPT no encontrado. ¿Es un savegame válido?"
        sys.exit(msg)

    expected = dim_x * dim_y
    print(f"  Mapa: {dim_x} × {dim_y} = {expected:,} teselas")

    mapt = chunks.get("MAPT", b"")
    map5 = chunks.get("MAP5", b"")
    map5 = map5[:expected].ljust(expected, b"\x00")
    water = water_tile_type_counts(mapt[:expected], map5)
    if water["tiles"]:
        print(
            f"  Agua: {water['tiles']:,} teselas — Clear {water['clear']:,}, "
            f"Coast {water['coast']:,}, otro {water['other']:,}"
        )

    obj_types: dict[int, int] = chunks.get("OBJS", {})  # type: ignore[assignment]
    if obj_types:
        n_fixed = sum(
            1
            for i in range(expected)
            if (mapt[i] >> 4) & 0xF == 10 and obj_types.get(i, 0xFF) != 0xFF
        )
        if n_fixed:
            print(f"  Objetos con tipo resuelto desde OBJS: {n_fixed}")

    map2 = chunks.get("MAP2", b"")
    if len(map2) >= 2 * expected:
        print(f"  MAP2 u16: plano bajo+alto ({2 * expected:,} bytes en save → .ottdmap v5+12)")

    if version < SLV_INCREASE_HOUSE_LIMIT:
        n_legacy = sum(1 for i in range(expected) if ((mapt[i] >> 4) & 0xF) == MP_HOUSE)
        print(
            f"  HouseID desde M3HI/M3LO (save < {SLV_INCREASE_HOUSE_LIMIT}): "
            f"{n_legacy:,} teselas MP_HOUSE"
        )

    body = export_ottdmap_from_chunks(chunks, version)
    tile_types = mapt[:expected]

    indp_pairs: list[tuple[int, int]] = []
    if "INDY" in chunks and isinstance(chunks["INDY"], list):
        indp_pairs = chunks["INDY"]  # type: ignore[assignment]
    if indp_pairs:
        print(f"  INDP: {len(indp_pairs)} industrias (índice → tipo)")

    stnn_blob = chunks.get("STNN", b"")
    if isinstance(stnn_blob, (bytes, bytearray)) and stnn_blob:
        print(f"  STNN: blob {len(stnn_blob):,} bytes")

    tnbp_blob = b""
    for _k in ("TNBP", "TBUS", "TUNN"):
        b = chunks.get(_k, b"")
        if isinstance(b, (bytes, bytearray)) and b:
            tnbp_blob = bytes(b)
            break
    if tnbp_blob:
        print(f"  TNBP: blob {len(tnbp_blob):,} bytes")

    stxy = build_stxy_footer(tile_types, dim_x, dim_y)
    n_stxy = struct.unpack_from("<I", stxy, 4)[0]
    if n_stxy:
        print(f"  STXY: {n_stxy} teselas MP_STATION (x,y)")

    out_path.write_bytes(body)
    print(f"✓ Escrito: {out_path}  ({out_path.stat().st_size:,} bytes)  [v5+12 densidad + footers]")

    type_counts: dict[int, int] = {}
    for b in tile_types:
        t = (b >> 4) & 0xF
        type_counts[t] = type_counts.get(t, 0) + 1

    type_names = {
        0: "Clear",
        1: "Railway",
        2: "Road",
        3: "House",
        4: "Trees",
        5: "Station",
        6: "Water",
        7: "Void",
        8: "Industry",
        9: "Tunnelbridge",
        10: "Object",
    }
    print("\nDistribución de tipos de tesela:")
    for t, count in sorted(type_counts.items()):
        name = type_names.get(t, f"Unknown({t})")
        pct = count / expected * 100
        print(f"  {t:2d}  {name:<14}  {count:>8,}  ({pct:.1f}%)")


if __name__ == "__main__":
    main()
