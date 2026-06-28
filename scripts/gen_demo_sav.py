#!/usr/bin/env python3
"""
Genera un savegame sintético de OpenTTD (.sav, contenedor OTTN sin compresión)
para probar el parser nativo de openttdrs (`openttdrs-core/src/sav/`).

Contenido del mapa (64×64):
  - anillo de agua en el borde
  - «Villa Demo»: cruce de carreteras, casas y una parada de bus
  - línea férrea horizontal con la estación «Central Demo»
  - «Puerto Sur»: caserío secundario
  - mina de carbón 2×2 con su registro INDY
  - chunks STNN (estaciones), CITY (ciudades), INDY (industrias),
    PLYR (dinero de la empresa) y VEHS (un tren y un bus)

Uso:
  python3 scripts/gen_demo_sav.py [salida.sav]   (default: save/demo_openttd.sav)

Fixture de CI/tests:
  python3 scripts/gen_demo_sav.py crates/openttdrs-core/tests/fixtures/demo_openttd.sav
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

W = H = 64
N = W * H
SAVE_VERSION = 350  # ≥ 295: chunks de tabla; ≥ 348: m8 directo

# Tipos MAPT (nibble alto)
MP_CLEAR = 0
MP_RAILWAY = 1
MP_ROAD = 2
MP_HOUSE = 3
MP_STATION = 5
MP_WATER = 6
MP_INDUSTRY = 8

# RoadBits (m5 en MP_ROAD): NW=1, SW=2, SE=4, NE=8
ROAD_X = 2 | 8
ROAD_Y = 1 | 4
ROAD_CROSS = 0x0F
# TrackBits (m5 en MP_RAILWAY)
TRACK_X = 1
# StationType en m6 (bits 3..6)
ST_RAIL = 0 << 3
ST_BUS = 3 << 3

CH_RIFF = 0
CH_TABLE = 3
CH_SPARSE_TABLE = 4


def write_gamma(v: int, buf: bytearray) -> None:
    assert v < (1 << 14), "el generador usa gammas pequeños"
    if v < (1 << 7):
        buf.append(v)
    else:
        buf.append(0x80 | (v >> 8))
        buf.append(v & 0xFF)


def write_str(s: str, buf: bytearray) -> None:
    raw = s.encode("utf-8")
    write_gamma(len(raw), buf)
    buf.extend(raw)


def riff_chunk(name: bytes, payload: bytes) -> bytes:
    size = len(payload)
    out = bytearray(name)
    out.append(((size >> 24) << 4) | CH_RIFF)
    out.append((size >> 16) & 0xFF)
    out.append((size >> 8) & 0xFF)
    out.append(size & 0xFF)
    out.extend(payload)
    return bytes(out)


def table_chunk(name: bytes, fields: list[tuple[int, str]], records: list[bytes]) -> bytes:
    header = bytearray()
    for ftype, key in fields:
        header.append(ftype)
        write_str(key, header)
    header.append(0)
    return raw_table_chunk(name, bytes(header), records, CH_TABLE)


def raw_table_chunk(
    name: bytes, header: bytes, records: list[bytes], ch_type: int
) -> bytes:
    """Chunk de tabla con header arbitrario (permite structs anidados)."""
    out = bytearray(name)
    out.append(ch_type)
    write_gamma(len(header) + 1, out)
    out.extend(header)
    for rec in records:
        write_gamma(len(rec) + 1, out)
        out.extend(rec)
    write_gamma(0, out)
    return bytes(out)


def idx(x: int, y: int) -> int:
    return y * W + x


def build_map_planes() -> tuple[bytearray, bytearray, bytearray, bytearray, bytearray]:
    mapt = bytearray([MP_CLEAR << 4]) * 1
    mapt = bytearray(N)
    maph = bytearray(N)  # plano: altura 0 uniforme
    m5 = bytearray(N)
    m6 = bytearray(N)
    m8 = bytearray(N * 2)  # LE u16 por tesela

    def set_tile(x: int, y: int, t: int, m5v: int = 0, m6v: int = 0, m8v: int = 0) -> None:
        i = idx(x, y)
        mapt[i] = (t << 4) & 0xFF
        m5[i] = m5v & 0xFF
        m6[i] = m6v & 0xFF
        struct.pack_into("<H", m8, i * 2, m8v & 0xFFFF)

    # Anillo de agua (2 teselas).
    for y in range(H):
        for x in range(W):
            if x < 2 or y < 2 or x >= W - 2 or y >= H - 2:
                set_tile(x, y, MP_WATER)

    # Villa Demo: cruce de carreteras (fila y=16, columna x=16).
    for x in range(10, 23):
        set_tile(x, 16, MP_ROAD, m5v=ROAD_X)
    for y in range(10, 23):
        set_tile(16, y, MP_ROAD, m5v=ROAD_Y)
    set_tile(16, 16, MP_ROAD, m5v=ROAD_CROSS)

    # Casas alrededor (HouseID < 110 = set base).
    houses = [
        (14, 14, 6), (15, 14, 7), (17, 14, 8), (18, 14, 9),
        (14, 15, 10), (18, 15, 11), (13, 17, 12), (14, 18, 13),
        (17, 18, 14), (18, 17, 15), (15, 18, 16), (19, 16, 17),
        (12, 16, 18), (16, 12, 19), (16, 20, 20),
    ]
    for x, y, hid in houses:
        set_tile(x, y, MP_HOUSE, m8v=hid)

    # Parada de bus junto a la carretera (al sur de la fila y=16).
    set_tile(17, 15, MP_STATION, m6v=ST_BUS)

    # Línea férrea horizontal y estación de tren al norte de la vía.
    for x in range(8, 49):
        set_tile(x, 40, MP_RAILWAY, m5v=TRACK_X)
    set_tile(28, 39, MP_STATION, m6v=ST_RAIL)

    # Puerto Sur: caserío secundario.
    for x, y, hid in [(44, 46, 21), (45, 46, 22), (44, 47, 23), (46, 47, 24)]:
        set_tile(x, y, MP_HOUSE, m8v=hid)
    for x in range(43, 48):
        set_tile(x, 48, MP_ROAD, m5v=ROAD_X)

    # Mina de carbón 2×2 al norte de la vía (gfx 0..3 = coal mine).
    for i, (x, y) in enumerate([(36, 20), (37, 20), (36, 21), (37, 21)]):
        set_tile(x, y, MP_INDUSTRY, m5v=i)

    return mapt, maph, m5, m6, m8


def build_sav() -> bytes:
    mapt, maph, m5, m6, m8 = build_map_planes()

    data = bytearray()

    # MAPS como RIFF (dims BE), igual que los saves clásicos.
    data.extend(riff_chunk(b"MAPS", struct.pack(">II", W, H)))
    data.extend(riff_chunk(b"MAPT", bytes(mapt)))
    data.extend(riff_chunk(b"MAPH", bytes(maph)))
    data.extend(riff_chunk(b"MAPO", bytes(N)))
    data.extend(riff_chunk(b"MAP2", bytes(N * 2)))
    data.extend(riff_chunk(b"M3LO", bytes(N)))
    data.extend(riff_chunk(b"M3HI", bytes(N)))
    data.extend(riff_chunk(b"MAP5", bytes(m5)))
    data.extend(riff_chunk(b"MAPE", bytes(m6)))
    data.extend(riff_chunk(b"MAP7", bytes(N)))
    data.extend(riff_chunk(b"MAP8", bytes(m8)))

    # STNN: estaciones con nombre (facilities: 1 tren, 4 bus).
    st_fields = [(6, "xy"), (10 | 0x10, "name"), (2, "facilities")]
    st1 = bytearray()
    st1.extend(struct.pack(">I", idx(28, 39)))
    write_str("Central Demo", st1)
    st1.append(0x01)
    st2 = bytearray()
    st2.extend(struct.pack(">I", idx(17, 15)))
    write_str("Parada Villa Demo", st2)
    st2.append(0x04)
    data.extend(table_chunk(b"STNN", st_fields, [bytes(st1), bytes(st2)]))

    # CITY: ciudades con población. La segunda no tiene nombre custom: usa el
    # generador nativo (townnametype 0x20C0 = inglés original, seed fijo).
    city_fields = [
        (6, "xy"),
        (10 | 0x10, "name"),
        (6, "cache.population"),
        (6, "townnamegrfid"),
        (4, "townnametype"),
        (6, "townnameparts"),
    ]
    t1 = bytearray()
    t1.extend(struct.pack(">I", idx(16, 16)))
    write_str("Villa Demo", t1)
    t1.extend(struct.pack(">I", 1200))
    t1.extend(struct.pack(">I", 0))
    t1.extend(struct.pack(">H", 0x20C0))
    t1.extend(struct.pack(">I", 0))
    t2 = bytearray()
    t2.extend(struct.pack(">I", idx(45, 47)))
    write_str("", t2)
    t2.extend(struct.pack(">I", 350))
    t2.extend(struct.pack(">I", 0))
    t2.extend(struct.pack(">H", 0x20C0))  # SPECSTR_TOWNNAME_START: inglés original
    t2.extend(struct.pack(">I", 0x51E2A37C))  # seed → nombre generado estilo OpenTTD
    data.extend(table_chunk(b"CITY", city_fields, [bytes(t1), bytes(t2)]))

    # INDY: mina de carbón 2×2 (type 0 = coal mine).
    indy_fields = [(6, "location.tile"), (2, "location.w"), (2, "location.h"), (2, "type")]
    ind = bytearray()
    ind.extend(struct.pack(">I", idx(36, 20)))
    ind.append(2)
    ind.append(2)
    ind.append(0)
    data.extend(table_chunk(b"INDY", indy_fields, [bytes(ind)]))

    # PLYR: dinero de la empresa del jugador.
    pl = bytearray()
    pl.extend(struct.pack(">q", 250_000))
    data.extend(table_chunk(b"PLYR", [(7, "money")], [bytes(pl)]))

    # ORDL: listas de órdenes (tren → estación 0, bus → estación 1).
    ordl_header = bytearray()
    ordl_header.append(0x1B)
    write_str("orders", ordl_header)
    ordl_header.append(0)
    ordl_header.append(2)
    write_str("type", ordl_header)
    ordl_header.append(2)
    write_str("flags", ordl_header)
    ordl_header.append(4)
    write_str("dest", ordl_header)
    ordl_header.append(2)
    write_str("refit_cargo", ordl_header)
    ordl_header.append(4)
    write_str("wait_time", ordl_header)
    ordl_header.append(4)
    write_str("travel_time", ordl_header)
    ordl_header.append(4)
    write_str("max_speed", ordl_header)
    ordl_header.append(0)

    def goto_station_order(station_id: int) -> bytes:
        o = bytearray()
        o.append(1)  # OT_GOTO_STATION
        o.append(0)
        o.extend(struct.pack(">H", station_id))
        o.append(0xFF)
        o.extend(struct.pack(">H", 0))
        o.extend(struct.pack(">H", 0))
        o.extend(struct.pack(">H", 0))
        return bytes(o)

    def ordl_record(order: bytes) -> bytes:
        rec = bytearray()
        rec.append(1)  # orders ×1
        rec.extend(order)
        return bytes(rec)

    data.extend(
        raw_table_chunk(
            b"ORDL",
            bytes(ordl_header),
            [ordl_record(goto_station_order(0)), ordl_record(goto_station_order(1))],
            CH_TABLE,
        )
    )

    # VEHS (sparse): un tren en la vía y un bus en la carretera, ambos cabeza
    # de convoy (subtype bit 0 = GVSF_FRONT). Header con structs anidados:
    # type u8 + train/roadveh { common { tile, subtype, cargo_type } }.
    vehs_header = bytearray()
    vehs_header.append(2)
    write_str("type", vehs_header)
    vehs_header.append(11 | 0x10)
    write_str("train", vehs_header)
    vehs_header.append(11 | 0x10)
    write_str("roadveh", vehs_header)
    vehs_header.append(0)
    for _ in range(2):  # sub-listas de train y roadveh (depth-first)
        vehs_header.append(11 | 0x10)
        write_str("common", vehs_header)
        vehs_header.append(0)
        vehs_header.append(6)
        write_str("tile", vehs_header)
        vehs_header.append(2)
        write_str("subtype", vehs_header)
        vehs_header.append(2)
        write_str("cargo_type", vehs_header)
        vehs_header.append(6)
        write_str("orders", vehs_header)
        vehs_header.append(2)
        write_str("cur_real_order_index", vehs_header)
        vehs_header.append(2)
        write_str("vehstatus", vehs_header)
        vehs_header.append(0)

    def vehs_common(tile: int, subtype: int, cargo: int, order_list: int, buf: bytearray) -> None:
        buf.append(1)  # train/roadveh presente
        buf.append(1)  # common presente
        buf.extend(struct.pack(">I", tile))
        buf.append(subtype)
        buf.append(cargo)
        buf.extend(struct.pack(">I", order_list))  # OrderList ref = índice + 1
        buf.append(0)  # cur_real_order_index
        buf.append(0)  # vehstatus: running

    v_train = bytearray([0])  # índice sparse 0
    v_train.append(0)  # type 0 = tren
    vehs_common(idx(20, 40), 0x01, 1, 1, v_train)  # lista 0 → ref 1; cargo carbón
    v_train.append(0)  # roadveh ausente

    v_bus = bytearray([1])  # índice sparse 1
    v_bus.append(1)  # type 1 = carretera
    v_bus.append(0)  # train ausente
    vehs_common(idx(13, 16), 0x01, 0, 2, v_bus)  # lista 1 → ref 2; pasajeros

    data.extend(
        raw_table_chunk(b"VEHS", bytes(vehs_header), [bytes(v_train), bytes(v_bus)], CH_SPARSE_TABLE)
    )

    data.extend(b"\x00\x00\x00\x00")  # terminador de stream

    out = bytearray(b"OTTN")
    out.extend(struct.pack(">H", SAVE_VERSION))
    out.extend(b"\x00\x00")
    out.extend(data)
    return bytes(out)


def main() -> None:
    out_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("save/demo_openttd.sav")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_bytes(build_sav())
    print(f"✓ Escrito: {out_path} ({out_path.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
