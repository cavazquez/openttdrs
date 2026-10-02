// Instantiates pristine OpenTTD AnimationBase and the default airport table.
// Only world interfaces (frame storage, clock, dirty marks) are adapted.
#include <algorithm>
#include <array>
#include <cassert>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include "oracle-include/newgrf_animation_type.h"
using TileIndex = unsigned;
using CallbackID = unsigned;
constexpr uint16_t CALLBACK_FAILED = 0xffff;
constexpr unsigned STR_NULL = 0;
constexpr unsigned INVALID_AIRPORTTILE = 0xffff;
constexpr unsigned NEW_AIRPORTTILE_OFFSET = 74;
struct AirportAnimationTriggers {};
struct AirportTileCallbackMasks { bool Test(unsigned) const { return false; } };
struct GrfFile { unsigned grf_version = 8; };
struct SubstituteGRFFileProps {
 unsigned substitute; GrfFile *grffile = nullptr; unsigned grfid = 0;
 constexpr explicit SubstituteGRFFileProps(unsigned id): substitute(id) {}
};
struct AirportTileSpec {
 AnimationInfo<AirportAnimationTriggers> animation; unsigned name;
 AirportTileCallbackMasks callback_mask; unsigned animation_special_flags;
 bool enabled; SubstituteGRFFileProps grf_prop; std::array<unsigned, 0> unused;
};
template<class T, size_t N> constexpr size_t lengthof(const T (&)[N]) { return N; }
struct Station {};
struct TimerGameTick { static inline uint64_t counter = 0; };
struct { struct { bool ambient = false; } sound; } _settings_client;
static std::array<uint8_t, 6> frame{};
static std::array<bool, 6> dirty{};
static unsigned random_calls = 0;
uint8_t GetAnimationFrame(TileIndex tile) { return frame.at(tile); }
void SetAnimationFrame(TileIndex tile, uint8_t value) { frame.at(tile) = value; }
void MarkTileDirtyByTile(TileIndex tile) { dirty.at(tile) = true; }
void AddAnimatedTile(TileIndex, bool) {}
void DeleteAnimatedTile(TileIndex) {}
uint32_t Random() { ++random_calls; return 0; }
template<class T> auto GB(T value, unsigned start, unsigned bits) { return (value >> start) & ((1u << bits) - 1); }
template<class T> auto Clamp(T value, int lower, int upper) { return std::clamp(static_cast<int>(value), lower, upper); }
void ErrorUnknownCallbackResult(unsigned, unsigned, unsigned) { std::abort(); }
void PlayTileSound(GrfFile *, unsigned, TileIndex) { std::abort(); }
#include "native-src__newgrf_animation_base.h"
#include "native-src__table__airporttiles.h"
uint16_t NoCallback(CallbackID, uint32_t, uint32_t, const AirportTileSpec *, Station *, TileIndex, int) { std::abort(); }
struct AirportOracle : AnimationBase<AirportOracle, AirportTileSpec, Station, int, NoCallback, TileAnimationFrameAnimationHelper<Station>> {
 static constexpr unsigned cb_animation_speed = 0x154;
 static constexpr unsigned cb_animation_next_frame = 0x153;
 static constexpr unsigned cbm_animation_speed = 1;
 static constexpr unsigned cbm_animation_next_frame = 2;
};
int main() {
 constexpr std::array<unsigned, 6> gfx{31, 51, 52, 39, 73, 47};
 std::cout << "tick,gfx,frame,dirty,random_calls\n";
 Station station;
 for (uint64_t tick = 1; tick <= 48; ++tick) {
  TimerGameTick::counter = tick; dirty.fill(false);
  for (unsigned tile = 0; tile < gfx.size(); ++tile) {
   const auto &spec = _origin_airporttile_specs[gfx[tile]];
   if (spec.animation.status != AnimationStatus::NoAnimation) AirportOracle::AnimateTile(&spec, &station, tile, false);
   std::cout << tick << ',' << gfx[tile] << ',' << unsigned(frame[tile]) << ',' << dirty[tile] << ',' << random_calls << '\n';
  }
 }
 assert(random_calls == 0);
}
