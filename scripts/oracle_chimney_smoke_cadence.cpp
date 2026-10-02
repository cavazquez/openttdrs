// Native bodies are extracted unchanged from the pinned GPL-2.0 OpenTTD source.
#include <array>
#include <cassert>
#include <cstdint>
#include <iostream>
using SpriteID = uint32_t;
using TileIndex = int;
constexpr int MP_INDUSTRY = 1;
struct DrawLayer { SpriteID sprite = 0; };
struct SpriteSeq {
    std::array<DrawLayer, 1> seq{};
    void Set(SpriteID sprite) { seq[0].sprite = sprite; }
};
struct EffectVehicle {
    struct { SpriteSeq sprite_seq; } sprite_cache;
    unsigned progress = 7;
    int x_pos = 0, y_pos = 0;
    unsigned updates = 0;
    void UpdatePositionAndViewport() { ++updates; }
};
TileIndex TileVirtXY(int, int) { return 0; }
bool IsTileType(TileIndex, int) { return true; }
#include "native-constants.inc"
#include "native-increment.inc"
#include "native-chimney-tick.inc"
int main() {
    std::cout << "phase,tick,sprite_id,frame,progress,position_updates\n";
    for (unsigned phase = 0; phase < 8; ++phase) {
        auto *v = new EffectVehicle();
        v->sprite_cache.sprite_seq.Set(SPR_CHIMNEY_SMOKE_0 + phase);
        for (unsigned tick = 0; tick <= 64; ++tick) {
            if (tick > 0) assert(ChimneySmokeTick(v));
            std::cout << phase << ',' << tick << ',' << v->sprite_cache.sprite_seq.seq[0].sprite
                      << ',' << v->sprite_cache.sprite_seq.seq[0].sprite - SPR_CHIMNEY_SMOKE_0
                      << ',' << v->progress << ',' << v->updates << '\n';
        }
        delete v;
    }
}
