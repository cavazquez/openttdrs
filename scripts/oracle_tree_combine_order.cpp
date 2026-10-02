// World adapter for one captured vanilla tree tile; no raster or general map decoding.

#include <algorithm>
#include <cassert>
#include <bit>
#include <cstdint>
#include <iostream>
#include <iterator>
using uint=unsigned int;
using SpriteID=uint32_t;
using PaletteID=uint32_t;
struct PalSpriteID { SpriteID sprite; PaletteID pal; };
struct Vec3 {int x=0,y=0,z=0;};
struct SpriteBounds {Vec3 origin,extent,offset;};
struct TileIndex {uint value; uint base() const {return value;}};
struct TileInfo {int x,y,z,tileh;TileIndex tile;};
constexpr int TREE_GROUND_GRASS=0,TREE_GROUND_ROUGH=1,TREE_GROUND_SNOW_DESERT=2,TREE_GROUND_SHORE=3,TREE_GROUND_ROUGH_SNOW=4;
constexpr int TREE_SUB_ARCTIC=12,TREE_RAINFOREST=20,TILE_SIZE=16,TO_TREES=0;
constexpr SpriteID _clear_land_sprites_snow_desert[4]={};
uint emitted=0;
uint GetTreeGround(TileIndex) {return 0;}
uint GetTreeDensity(TileIndex) {return 3;}
uint GetTreeType(TileIndex) {return 7;}
uint GetTreeCount(TileIndex) {return 4;}
uint GetTreeGrowth(TileIndex) {return 0;}
uint to_underlying(uint value) {return value;}
uint CountBits(uint value) {return std::popcount(value);}
uint GB(uint value,uint start,uint count) {return (value>>start)&((1u<<count)-1);}
bool IsInsideMM(uint value,uint start,uint end) {return value>=start&&value<end;}
bool IsInvisibilitySet(int) {return false;}
bool IsTransparencySet(int) {return false;}
void DrawShoreTile(int) {}
void DrawClearLandTile(TileInfo*,uint) {}
void DrawHillyLandTile(TileInfo*) {}
void DrawGroundSprite(SpriteID,PaletteID) {}
int SlopeToSpriteOffset(int) {return 0;}
int GetSlopeMaxPixelZ(int slope) {assert(slope==4);return 8;}
void StartSpriteCombine() {emitted=0;}
void EndSpriteCombine() {}
template<class T,std::size_t N> constexpr std::size_t lengthof(T(&)[N]) {return N;}
void AddSortableSpriteToDraw(SpriteID sprite,PaletteID pal,int,int,int,SpriteBounds bounds,bool) {
 std::cout<<emitted++<<','<<sprite<<','<<pal<<','<<bounds.offset.x<<','<<bounds.offset.y<<'\n';
}

#include "native-palettes.inc"
#include "native-tree-table.h"
#include "native-tree-list.inc"
#include "native-draw-tree.inc"
int main() {
 std::cout << "draw_order,sprite_id,palette_id,offset_x,offset_y\n";
 TileInfo ti{2144,1536,8,4,TileIndex{96*256+134}};
 DrawTile_Trees(&ti);
}
