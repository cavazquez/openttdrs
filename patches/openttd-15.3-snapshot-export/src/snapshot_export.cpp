/*
 * Snapshot export for openttdrs parity (#110).
 * Produces JSON without invoking openttdrs parsers.
 */

#include "snapshot_export.h"

#include "map_func.h"
#include "openttd.h"
#include "tile_map.h"
#include "tile_type.h"

#include "3rdparty/nlohmann/json.hpp"

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <map>
#include <queue>
#include <string>
#include <utility>
#include <vector>

namespace {

constexpr uint64_t FNV_OFFSET = 0xcbf29ce484222325ULL;
constexpr uint64_t FNV_PRIME = 0x100000001b3ULL;

struct Fnv1a64 {
	uint64_t h = FNV_OFFSET;
	void write_u8(uint8_t v)
	{
		h ^= v;
		h *= FNV_PRIME;
	}
	void write_u16(uint16_t v)
	{
		write_u8(static_cast<uint8_t>(v & 0xFF));
		write_u8(static_cast<uint8_t>((v >> 8) & 0xFF));
	}
	std::string hex() const
	{
		char buf[17];
		std::snprintf(buf, sizeof(buf), "%016llx", static_cast<unsigned long long>(h));
		return buf;
	}
};

/** Same mapping as openttdrs `ottd_tile_kind` + snapshot_dumper kind codes. */
uint8_t KindCode(Tile tile)
{
	const uint8_t ottd_type = static_cast<uint8_t>(GetTileType(tile));
	const uint8_t m5 = tile.m5();
	const uint8_t transport_subtype = (m5 >> 6) & 0x3;
	switch (ottd_type) {
		case MP_CLEAR:
		case MP_OBJECT:
			return 1; /* Grass */
		case MP_RAILWAY:
			return transport_subtype == 3 ? 11 : 4; /* RailDepot / Rail */
		case MP_ROAD:
			return transport_subtype == 2 ? 10 : 3; /* RoadDepot / Road */
		case MP_HOUSE:
			return 5;
		case MP_TREES:
			return 8; /* Forest */
		case MP_STATION:
			/* Alineado con openttdrs `ottd_tile_kind`: no distingue aeropuerto. */
			return 7;
		case MP_WATER:
			/* Alineado con openttdrs: depósitos navales cuentan como Water. */
			return 2;
		case MP_VOID:
			return 0;
		case MP_INDUSTRY:
			return 6;
		case MP_TUNNELBRIDGE: {
			const bool is_bridge = (m5 & 0x80) != 0;
			const uint8_t transport = (m5 >> 2) & 0x3;
			if (is_bridge) return transport == 0 ? 15 : 14; /* RailBridge / RoadBridge */
			return transport == 0 ? 13 : 12; /* RailTunnel / RoadTunnel */
		}
		default:
			return static_cast<uint8_t>(128u + ottd_type);
	}
}

const char *KindName(uint8_t code)
{
	switch (code) {
		case 0: return "Void";
		case 1: return "Grass";
		case 2: return "Water";
		case 3: return "Road";
		case 4: return "Rail";
		case 5: return "House";
		case 6: return "Industry";
		case 7: return "Station";
		case 8: return "Forest";
		case 9: return "CoalField";
		case 10: return "RoadDepot";
		case 11: return "RailDepot";
		case 12: return "RoadTunnel";
		case 13: return "RailTunnel";
		case 14: return "RoadBridge";
		case 15: return "RailBridge";
		case 16: return "ShipDepot";
		case 17: return "Airport";
		default: return "Unknown";
	}
}

bool IsKind(Tile tile, uint8_t want)
{
	return KindCode(tile) == want;
}

size_t CountComponents(uint8_t want)
{
	const uint w = Map::SizeX();
	const uint h = Map::SizeY();
	std::vector<uint8_t> visited(static_cast<size_t>(w) * h, 0);
	size_t comps = 0;
	auto idx = [w](uint x, uint y) { return static_cast<size_t>(y) * w + x; };

	for (uint y = 0; y < h; y++) {
		for (uint x = 0; x < w; x++) {
			const size_t i = idx(x, y);
			if (visited[i]) continue;
			Tile t(TileXY(x, y));
			if (!IsKind(t, want)) {
				visited[i] = 1;
				continue;
			}
			comps++;
			visited[i] = 1;
			std::queue<std::pair<uint, uint>> q;
			q.push({x, y});
			while (!q.empty()) {
				auto [cx, cy] = q.front();
				q.pop();
				const int nbs[4][2] = {{-1, 0}, {1, 0}, {0, -1}, {0, 1}};
				for (auto &d : nbs) {
					const int nx = static_cast<int>(cx) + d[0];
					const int ny = static_cast<int>(cy) + d[1];
					if (nx < 0 || ny < 0 || static_cast<uint>(nx) >= w || static_cast<uint>(ny) >= h) continue;
					const size_t ni = idx(static_cast<uint>(nx), static_cast<uint>(ny));
					if (visited[ni]) continue;
					visited[ni] = 1;
					if (IsKind(Tile(TileXY(static_cast<uint>(nx), static_cast<uint>(ny))), want)) {
						q.push({static_cast<uint>(nx), static_cast<uint>(ny)});
					}
				}
			}
		}
	}
	return comps;
}

} // namespace

bool OpenttdrsMaybeExportSnapshot(const std::string &source_path)
{
	const char *out_path = std::getenv("OPENTTDRS_SNAPSHOT_OUT");
	if (out_path == nullptr || out_path[0] == '\0') return true;

	/* Dedicated + `-g` genera un new-game (1er AfterLoadGame) y luego carga el .sav
	 * (2º). Por defecto saltamos el primero. OPENTTDRS_SNAPSHOT_MIN_CALL=1 fuerza el 1º. */
	static int call_count = 0;
	call_count++;
	const char *min_s = std::getenv("OPENTTDRS_SNAPSHOT_MIN_CALL");
	const int min_call = (min_s != nullptr && min_s[0] != '\0') ? std::atoi(min_s) : 2;
	if (call_count < min_call) return true;

	const char *commit = std::getenv("OPENTTDRS_OPENTTD_COMMIT");
	const uint w = Map::SizeX();
	const uint h = Map::SizeY();

	Fnv1a64 h_height, h_kind, h_mapt, h_rail, h_road;
	std::map<std::string, uint64_t> counts;
	uint8_t min_h = 255, max_h = 0;

	for (uint y = 0; y < h; y++) {
		for (uint x = 0; x < w; x++) {
			Tile t(TileXY(x, y));
			const uint8_t height = static_cast<uint8_t>(TileHeight(t));
			const uint8_t mapt = t.type(); /* full MAPT byte */
			const uint8_t m5 = t.m5();
			const uint8_t kind = KindCode(t);
			const char *name = KindName(kind);
			counts[name]++;
			min_h = std::min(min_h, height);
			max_h = std::max(max_h, height);
			h_height.write_u8(height);
			h_kind.write_u8(kind);
			h_mapt.write_u8(mapt);
			if (kind == 4) { /* Rail (plain / signals share m5 track bits) */
				h_rail.write_u8(m5 & 0x3F);
				h_rail.write_u8(t.m3());
				h_rail.write_u8(t.m4()); /* ottdmap m3hi */
			}
			if (kind == 3) { /* Road */
				h_road.write_u8(m5 & 0x0F);
				h_road.write_u16(t.m8());
			}
		}
	}

	nlohmann::json j;
	j["schema_version"] = 1;
	j["producer"] = "openttd";
	j["openttd_commit"] = commit != nullptr ? commit : "";
	j["source_path"] = source_path;
	j["map"] = {
		{"width", w},
		{"height", h},
		{"tile_count", static_cast<uint64_t>(w) * h},
		{"tile_kind_counts", counts},
		{"max_height", max_h},
		{"min_height", min_h == 255 ? 0 : min_h},
	};
	j["hashes"] = {
		{"height_hash_fnv1a64", h_height.hex()},
		{"kind_hash_fnv1a64", h_kind.hex()},
		{"mapt_hash_fnv1a64", h_mapt.hex()},
		{"rail_bits_hash_fnv1a64", h_rail.hex()},
		{"road_bits_hash_fnv1a64", h_road.hex()},
	};
	/* ottdmap footers do not exist in the live engine; leave zeros. */
	j["extras"] = {
		{"dense_payload_end", 0},
		{"footer_industry_pairs", 0},
		{"footer_station_xy", 0},
		{"footer_tnbp_blob_len", 0},
	};
	j["components"] = {
		{"industry_components", CountComponents(6)},
		{"station_components", CountComponents(7)},
	};

	std::ofstream out(out_path);
	if (!out) return false;
	out << j.dump(2) << '\n';
	if (!out) return false;

	/* Dedicated: salir tras exportar (evita servidor colgado). */
	_exit_game = true;
	return true;
}
