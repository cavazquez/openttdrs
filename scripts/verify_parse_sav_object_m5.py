#!/usr/bin/env python3
"""Regresión de `MP_OBJECT`: MAP5 crudo y pool `OBJS` deben viajar separados.

OpenTTD forma `ObjectID = MAP2 | (MAP5 << 16)`. Por tanto, al convertir una
partida `.sav` a `.ottdmap`, no se puede reemplazar MAP5 por `ObjectType`.
Este check usa el fixture versionado con transmisor y faro, verifica ambos
planos byte a byte y exige que el footer `OBTY` mantenga el vínculo de pool.

Uso:

  python3 scripts/verify_parse_sav_object_m5.py
  python3 scripts/verify_parse_sav_object_m5.py ruta/partida.sav
"""

from __future__ import annotations

import importlib.util
import struct
import sys
from pathlib import Path


MP_OBJECT = 10


def _load_parse_sav(repo_root: Path):
    script = repo_root / "scripts" / "parse_sav.py"
    spec = importlib.util.spec_from_file_location("parse_sav", script)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"No se pudo cargar {script}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _dense_object_planes(data: bytes) -> tuple[int, int, bytes, bytes, bytes, bytes]:
    """Devuelve dimensiones y planos MAPT/MAP2/MAP2HI/MAP5 de un MAP1 v1."""
    if len(data) < 16 or data[:4] != b"MAP1":
        raise ValueError("cabecera MAP1 inválida")
    width, height = struct.unpack_from("<II", data, 4)
    version, flags = struct.unpack_from("<HH", data, 12)
    if version != 1:
        raise ValueError(f"format_version {version} no soportado")
    count = width * height
    base = 16
    has_m2_hi = flags & 1 != 0
    plane_count = 12 if has_m2_hi else 11
    dense_end = base + plane_count * count
    if len(data) < dense_end:
        raise ValueError("bloque denso MAP1 truncado")
    mapt = data[base : base + count]
    m2_start = base + 3 * count
    m2_lo = data[m2_start : m2_start + count]
    if has_m2_hi:
        m2_hi = data[m2_start + count : m2_start + 2 * count]
        m5_start = base + 7 * count
    else:
        m2_hi = bytes(count)
        m5_start = base + 6 * count
    return width, height, mapt, m2_lo, m2_hi, data[m5_start : m5_start + count]


def _obty_footer(data: bytes, width: int, height: int) -> dict[int, int] | None:
    """Lee `OBTY` sin aceptar magics desconocidos ni footers truncados."""
    if len(data) < 16:
        raise ValueError("MAP1 truncado")
    flags = struct.unpack_from("<H", data, 14)[0]
    count = width * height
    off = 16 + (12 if flags & 1 else 11) * count
    while off + 8 <= len(data):
        magic = data[off : off + 4]
        entries = struct.unpack_from("<I", data, off + 4)[0]
        off += 8
        if magic == b"INDP":
            size = entries * 3
        elif magic == b"OBTY":
            size = entries * 6
            if off + size > len(data):
                raise ValueError("footer OBTY truncado")
            return {
                struct.unpack_from("<I", data, off + i * 6)[0]: struct.unpack_from(
                    "<H", data, off + i * 6 + 4
                )[0]
                for i in range(entries)
            }
        elif magic in (b"STNN", b"TNBP"):
            size = entries
        elif magic == b"STXY":
            size = entries * 4
        else:
            raise ValueError(f"footer desconocido {magic!r}")
        if off + size > len(data):
            raise ValueError(f"footer {magic!r} truncado")
        off += size
    return None


def verify_sav_roundtrip(parse_sav, sav_path: Path) -> list[str]:
    raw = sav_path.read_bytes()
    data, version = parse_sav.decompress(raw)
    chunks = parse_sav.parse_chunks(data)
    width, height = parse_sav.dimensions_from_chunks(chunks)
    count = width * height
    src_mapt = chunks["MAPT"][:count]
    src_m5 = chunks.get("MAP5", b"")[:count].ljust(count, b"\0")
    src_map2 = chunks.get("MAP2", b"")[: 2 * count].ljust(2 * count, b"\0")
    object_types = chunks.get("OBJS")
    if not isinstance(object_types, dict):
        return [f"{sav_path.name}: falta tabla OBJS interpretable"]

    body = parse_sav.export_ottdmap_from_chunks(chunks, version)
    out_w, out_h, out_mapt, out_m2_lo, out_m2_hi, out_m5 = _dense_object_planes(body)
    if (out_w, out_h) != (width, height):
        return [f"{sav_path.name}: dimensiones MAP1 {out_w}×{out_h}, esperado {width}×{height}"]
    obty = _obty_footer(body, width, height)
    if obty is None:
        return [f"{sav_path.name}: falta footer OBTY"]

    errors: list[str] = []
    checked = 0
    for index, mapt in enumerate(src_mapt):
        if (mapt >> 4) & 0x0F != MP_OBJECT:
            continue
        checked += 1
        x, y = index % width, index // width
        # MAP2 en el chunk RIFF es `SLE_UINT16` big-endian; el exportador lo
        # separa a continuación en los planos bajo/alto de MAP1.
        src_object_id = int.from_bytes(src_map2[index * 2 : index * 2 + 2], "big") | (src_m5[index] << 16)
        out_object_id = out_m2_lo[index] | (out_m2_hi[index] << 8) | (out_m5[index] << 16)
        if out_mapt[index] != mapt:
            errors.append(f"({x},{y}) MAPT cambió: save=0x{mapt:02x} ottdmap=0x{out_mapt[index]:02x}")
        if out_m5[index] != src_m5[index]:
            errors.append(
                f"({x},{y}) MAP5 cambió: save=0x{src_m5[index]:02x} ottdmap=0x{out_m5[index]:02x}"
            )
        if out_object_id != src_object_id:
            errors.append(
                f"({x},{y}) ObjectID cambió: save={src_object_id} ottdmap={out_object_id}"
            )
        expected_type = object_types.get(src_object_id)
        if expected_type is None:
            errors.append(f"({x},{y}) ObjectID {src_object_id} no aparece en OBJS")
        elif obty.get(src_object_id) != expected_type:
            errors.append(
                f"({x},{y}) OBTY[{src_object_id}]={obty.get(src_object_id)!r}, "
                f"esperado OBJS={expected_type}"
            )
    if checked == 0:
        errors.append(f"{sav_path.name}: el fixture no contiene teselas MP_OBJECT")
    return [f"{sav_path.name}: {error}" for error in errors]


def main(argv: list[str] | None = None) -> int:
    repo = Path.cwd()
    parse_sav = _load_parse_sav(repo)
    args = list(argv or sys.argv[1:])
    sav_paths = [Path(arg) for arg in args] or [
        repo / "crates/openttdrs-core/tests/fixtures/train_dual_pbs_curve_15_3.sav"
    ]
    errors: list[str] = []
    for sav_path in sav_paths:
        if not sav_path.is_file():
            errors.append(f"no existe {sav_path}")
            continue
        errors.extend(verify_sav_roundtrip(parse_sav, sav_path))

    if errors:
        print("verify_parse_sav_object_m5: fallos:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return 1
    print("OK: MP_OBJECT conserva MAP5/ObjectID y OBTY resuelve ObjectType")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
