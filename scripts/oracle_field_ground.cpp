// Native bodies and tables are extracted unchanged from pinned GPL-2.0 OpenTTD.
#include <array>
#include <cassert>
#include <cstdint>
#include <iostream>
#include <stdexcept>
using uint = unsigned;
using SpriteID = uint32_t;
using TileIndex = int;
using Slope = uint8_t;
enum ClearGround { CLEAR_GRASS, CLEAR_ROUGH, CLEAR_ROCKS, CLEAR_FIELDS, CLEAR_SNOW, CLEAR_DESERT };
enum class GrfMiscBit { SecondRockyTileSet };
struct TileInfo { TileIndex tile = 0; Slope tileh = 0; uint x = 0, y = 0; };
constexpr uint PAL_NONE = 0;
unsigned crop_stage = 0, fence_calls = 0, ground_calls = 0;
SpriteID ground_sprite = 0;
ClearGround GetClearGround(TileIndex) { return CLEAR_FIELDS; }
bool IsSnowTile(TileIndex) { return false; }
uint8_t GetClearDensity(TileIndex) { throw std::logic_error("unexpected density branch"); }
uint GetFieldType(TileIndex) { return crop_stage; }
bool HasGrfMiscBit(GrfMiscBit) { return false; }
uint TileHash(uint, uint) { return 0; }
void DrawGroundSprite(SpriteID sprite, uint palette) { assert(palette == PAL_NONE); ground_sprite = sprite; ++ground_calls; }
void DrawClearLandFence(const TileInfo *) { ++fence_calls; }
void DrawClearLandTile(const TileInfo *, uint8_t) { throw std::logic_error("unexpected grass branch"); }
void DrawHillyLandTile(const TileInfo *) { throw std::logic_error("unexpected rough branch"); }
void DrawBridgeMiddle(TileInfo *, int) {} // Bridge geometry is outside this ground oracle.
constexpr std::array<SpriteID, 8> _clear_land_sprites_snow_desert{};
constexpr SpriteID SPR_OVERLAY_ROCKS_BASE = 0; // Unexercised snow branch.
#include "native-constants.inc"
#include "native-farmland.inc"
#include "native-slope-table.inc"
#include "native-slope-offset.inc"
#include "native-draw-clear.inc"
int main() {
    std::cout << "stage,tileh,slope_offset,ground_sprite_id,fence_calls\n";
    for (crop_stage = 0; crop_stage < 9; ++crop_stage) {
        for (unsigned slope = 0; slope < 32; ++slope) {
            TileInfo tile;
            tile.tileh = static_cast<Slope>(slope);
            fence_calls = ground_calls = 0;
            DrawTile_Clear(&tile);
            assert(ground_calls == 1 && fence_calls == 1);
            std::cout << crop_stage << ',' << slope << ',' << SlopeToSpriteOffset(tile.tileh)
                      << ',' << ground_sprite << ',' << fence_calls << '\n';
        }
    }
}
