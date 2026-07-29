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
  - chunks STNN moderno (SAVEBYTE+structs), CITY, INDY, PLYR, ORDL, VEHS (tren)

Nota (#226): OpenTTD 15.3 dedicated carga:
  - `mvp_openttd_stations.sav` (STNN sin VEHS)
  - `mvp_openttd_train.sav` (STNN + tren + ORDL; preferido vía cargo dump)
  - este demo (mapa rico + un tren; ROAD vehicles omitidos del export)

Preferido para regenerar fixtures OpenTTD-loadable:
  OPENTTDRS_DUMP_MVP_TRAIN_SAV=$PWD/crates/.../mvp_openttd_train.sav \\
    cargo test -p openttdrs-core --lib sav::write::tests::export_mvp_train_emits_vehs_ordl_and_direction -- --exact

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
SAVE_VERSION = 350  # ≥ 294 MAPS TABLE; ≥ 295 tablas; ≥ 348 m8 HouseID

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


INVALID_TILE = 0xFFFFFFFF
NUM_CARGO = 64
STR_SV_STNAME = 0x6006
VEH_INVALID = 0xFF


def _field(buf: bytearray, ftype: int, name: str) -> None:
    buf.append(ftype)
    write_str(name, buf)


def stnn_base_header(buf: bytearray) -> None:
    for ftype, name in (
        (6, "xy"),
        (6, "town"),
        (9, "string_id"),
        (0x1A, "name"),
        (2, "delete_ctr"),
        (2, "owner"),
        (2, "facilities"),
        (5, "build_date"),
        (4, "random_bits"),
        (2, "waiting_triggers"),
    ):
        _field(buf, ftype, name)
    buf.append(0)


def stnn_modern_header() -> bytes:
    """Header CH_TABLE alineado con OpenTTD 15.3 `station_sl.cpp` (SLV≥340)."""
    h = bytearray()
    for ftype, name in (
        (2, "facilities"),
        (0x1B, "normal"),
        (0x1B, "waypoint"),
        (0x1B, "speclist"),
        (0x1B, "roadstopspeclist"),
        (0x1B, "roadstoptiledata"),
    ):
        _field(h, ftype, name)
    h.append(0)

    # SlStationNormal
    _field(h, 0x1B, "base")
    for ftype, name in (
        (6, "train_station.tile"),
        (2, "train_station.w"),
        (2, "train_station.h"),
        (6, "bus_stops"),
        (6, "truck_stops"),
        (6, "ship_station.tile"),
        (2, "ship_station.w"),
        (2, "ship_station.h"),
        (6, "docking_station.tile"),
        (2, "docking_station.w"),
        (2, "docking_station.h"),
        (6, "airport.tile"),
        (2, "airport.w"),
        (2, "airport.h"),
        (2, "airport.type"),
        (2, "airport.layout"),
        (8, "airport.flags"),
        (2, "airport.rotation"),
        (6, "airport.psa"),
        (2, "indtype"),
        (2, "time_since_load"),
        (2, "time_since_unload"),
        (2, "last_vehicle_type"),
        (2, "had_vehicle_of_type"),
        (0x16, "loading_vehicles"),
        (8, "always_accepted"),
        (0x1B, "goods"),
    ):
        _field(h, ftype, name)
    h.append(0)

    stnn_base_header(h)

    # SlStationGoods
    for ftype, name in (
        (2, "status"),
        (2, "time_since_pickup"),
        (2, "rating"),
        (2, "last_speed"),
        (2, "last_age"),
        (2, "amount_fract"),
        (6, "cargo.reserved_count"),
        (4, "link_graph"),
        (4, "node"),
        (6, "max_waiting_cargo"),
        (0x1B, "flow"),
        (0x1B, "cargo"),
    ):
        _field(h, ftype, name)
    h.append(0)

    for ftype, name in ((4, "source"), (4, "via"), (6, "share"), (1, "restricted")):
        _field(h, ftype, name)
    h.append(0)

    _field(h, 4, "first")
    _field(h, 0x16, "second")
    h.append(0)

    # SlStationWaypoint
    _field(h, 0x1B, "base")
    for ftype, name in (
        (4, "town_cn"),
        (6, "train_station.tile"),
        (2, "train_station.w"),
        (2, "train_station.h"),
        (4, "waypoint_flags"),
        (6, "road_waypoint_area.tile"),
        (2, "road_waypoint_area.w"),
        (2, "road_waypoint_area.h"),
    ):
        _field(h, ftype, name)
    h.append(0)
    stnn_base_header(h)

    for _ in range(2):  # speclist + roadstopspeclist
        _field(h, 6, "grfid")
        _field(h, 4, "localidx")
        h.append(0)

    for ftype, name in ((6, "tile"), (2, "random_bits"), (2, "animation_frame")):
        _field(h, ftype, name)
    h.append(0)
    return bytes(h)


def _empty_goods(buf: bytearray) -> None:
    buf.extend([0, 255, 175, 0, 255, 0])
    buf.extend(struct.pack(">I", 0))
    buf.extend(struct.pack(">HH", 0xFFFF, 0xFFFF))
    buf.extend(struct.pack(">I", 0))
    write_gamma(0, buf)
    write_gamma(0, buf)


def stnn_normal_record(tile: int, name: str, facilities: int) -> bytes:
    rec = bytearray()
    rec.append(facilities & 0xFF)  # SAVEBYTE
    rec.append(1)  # normal presente
    rec.append(1)  # base presente
    rec.extend(struct.pack(">I", tile))
    rec.extend(struct.pack(">I", 1))  # town ref
    rec.extend(struct.pack(">H", STR_SV_STNAME))
    write_str(name, rec)
    rec.append(0)  # delete_ctr
    rec.append(0)  # owner
    rec.append(facilities & 0xFF)
    rec.extend(struct.pack(">i", 0))  # build_date
    rec.extend(struct.pack(">H", 0))
    rec.append(0)  # waiting_triggers

    if facilities & 0x01:
        rec.extend(struct.pack(">I", tile))
        rec.extend([1, 1])
    else:
        rec.extend(struct.pack(">I", INVALID_TILE))
        rec.extend([0, 0])

    rec.extend(struct.pack(">II", 0, 0))  # bus/truck stops null
    rec.extend(struct.pack(">I", INVALID_TILE))
    rec.extend([0, 0])
    rec.extend(struct.pack(">I", INVALID_TILE))
    rec.extend([0, 0])

    if facilities & 0x08:
        rec.extend(struct.pack(">I", tile))
        rec.extend([1, 1, 0, 0])
    else:
        rec.extend(struct.pack(">I", INVALID_TILE))
        rec.extend([0, 0, 0, 0])
    rec.extend(struct.pack(">Q", 0))
    rec.append(0)  # rotation
    rec.extend(struct.pack(">I", 0))  # psa
    rec.extend([0, 0, 0, VEH_INVALID, 0])
    write_gamma(0, rec)  # loading_vehicles
    rec.extend(struct.pack(">Q", 0))  # always_accepted
    write_gamma(NUM_CARGO, rec)
    for _ in range(NUM_CARGO):
        _empty_goods(rec)

    rec.append(0)  # waypoint ausente
    write_gamma(0, rec)
    write_gamma(0, rec)
    write_gamma(0, rec)
    return bytes(rec)


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

    # Línea férrea horizontal con estación sobre la vía (pathable para el tren).
    for x in range(8, 49):
        set_tile(x, 40, MP_RAILWAY, m5v=TRACK_X)
    set_tile(28, 40, MP_STATION, m6v=ST_RAIL, m5v=TRACK_X)

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

    # MAPS CH_TABLE (SLV ≥ 294): dim_x/dim_y U32 BE — alineado con map_sl.cpp.
    # Planos MAPT…MAP8 siguen RIFF.
    data.extend(
        table_chunk(
            b"MAPS",
            [(6, "dim_x"), (6, "dim_y")],
            [struct.pack(">II", W, H)],
        )
    )
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

    # STNN moderno (SAVEBYTE + structs) — ver station_sl.cpp / sav/write/entities.rs.
    data.extend(
        raw_table_chunk(
            b"STNN",
            stnn_modern_header(),
            [
                stnn_normal_record(idx(28, 40), "Central Demo", 0x01),
                stnn_normal_record(idx(17, 15), "Parada Villa Demo", 0x04),
            ],
            CH_TABLE,
        )
    )

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

    # DATE: calendario + tick (como sav/write/meta.rs).
    date = struct.pack(">i", 1950 * 365) + struct.pack(">Q", 0)
    data.extend(table_chunk(b"DATE", [(5, "date"), (8, "tick_counter")], [date]))

    # PLYR: dinero de la empresa del jugador.
    pl = bytearray()
    pl.extend(struct.pack(">q", 250_000))
    data.extend(table_chunk(b"PLYR", [(7, "money")], [bytes(pl)]))

    # ORDL: una lista (tren → estación rail 0). ROAD omitido (#226).
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
        # OT_GOTO_STATION | (OrderStopLocation::Middle << 4) = 0x11
        o.append(0x11)
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
            [ordl_record(goto_station_order(0))],
            CH_TABLE,
        )
    )

    # VEHS (sparse): un tren loadable OpenTTD 15.3 — schema mínimo #226
    # (direction/owner/engine_type/x_pos/y_pos/z_pos/track). ROAD omitido.
    def append_vehs_common_fields(hdr: bytearray) -> None:
        for ftype, name in [
            (2, "subtype"),
            (2, "owner"),
            (6, "tile"),
            (6, "x_pos"),
            (6, "y_pos"),
            (5, "z_pos"),
            (2, "direction"),
            (4, "engine_type"),
            (2, "vehstatus"),
            (2, "cargo_type"),
            (6, "orders"),
            (2, "cur_real_order_index"),
        ]:
            hdr.append(ftype)
            write_str(name, hdr)
        hdr.append(0)

    vehs_header = bytearray()
    vehs_header.append(2)
    write_str("type", vehs_header)
    vehs_header.append(0x1B)
    write_str("train", vehs_header)
    vehs_header.append(0x1B)
    write_str("roadveh", vehs_header)
    vehs_header.append(0)
    # train nest
    vehs_header.append(0x1B)
    write_str("common", vehs_header)
    vehs_header.append(2)
    write_str("track", vehs_header)
    vehs_header.append(0)
    append_vehs_common_fields(vehs_header)
    # roadveh stub nest
    vehs_header.append(0x1B)
    write_str("common", vehs_header)
    vehs_header.append(0)
    append_vehs_common_fields(vehs_header)

    train_tile = idx(20, 40)
    tx, ty = 20, 40
    v_train = bytearray()
    v_train.append(0)  # sparse index
    v_train.append(0)  # VEH_TRAIN
    v_train.append(1)  # train presente
    v_train.append(1)  # common presente
    v_train.append(0x09)  # GVSF_FRONT | GVSF_ENGINE
    v_train.append(0)  # owner company 0
    v_train.extend(struct.pack(">I", train_tile))
    v_train.extend(struct.pack(">I", tx * 16))  # x_pos
    v_train.extend(struct.pack(">I", ty * 16 + 8))  # y_pos
    v_train.extend(struct.pack(">i", 0))  # z_pos
    v_train.append(1)  # DIR_NE
    v_train.extend(struct.pack(">H", 0))  # Kirby Paul Tank
    v_train.append(0)  # vehstatus running
    v_train.append(1)  # cargo carbón
    v_train.extend(struct.pack(">I", 1))  # ORDL ref 1
    v_train.append(0)  # cur_real_order_index
    v_train.append(1)  # TRACK_BIT_X
    v_train.append(0)  # roadveh ausente

    data.extend(
        raw_table_chunk(b"VEHS", bytes(vehs_header), [bytes(v_train)], CH_SPARSE_TABLE)
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
