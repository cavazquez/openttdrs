#!/usr/bin/env python3
"""Run pristine 8bpp-simple Draw on captured alpha at all six native zooms.

The 66x52 inputs are replicated to a 4x root sprite by a scaffold. This checks
sampling and rounded dimensions, not the native decoder, Encode or SAV draw.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("directory", type=Path)
parser.add_argument("--reference", type=Path, default=Path("reference/openttd-15.3-oracle"))
args = parser.parse_args()
work = args.directory.resolve()
for name in ("parent", "child"):
    alpha = (work / f"{name}.alpha").read_bytes()
    assert len(alpha) == 66 * 52 and set(alpha) <= {0, 255}, "captured binary alpha required"
pin = subprocess.check_output(["git", "-C", str(args.reference), "rev-parse", "HEAD"], text=True).strip()
assert pin == "14ec60f248547d4d062a1160f0fc26d742319888"


def source(path: str) -> str:
    return subprocess.check_output(["git", "-C", str(args.reference), "show", f"HEAD:{path}"], text=True)


blitter = source("src/blitter/8bpp_simple.cpp")
zoom = source("src/zoom_func.h")
draw = blitter.split("void Blitter_8bppSimple::Draw(", 1)[1].split("\nSprite *Blitter_8bppSimple::Encode(", 1)[0]
draw = "void Blitter_8bppSimple::Draw(" + draw
helpers = []
for name in ("ScaleByZoom", "UnScaleByZoom"):
    helpers.append("inline int " + name + zoom.split("inline int " + name, 1)[1].split("\n}", 1)[0] + "\n}\n")
preamble = r'''
#include <array>
#include <cassert>
#include <cstdint>
#include <fstream>
#include <iostream>
#include <iterator>
#include <string>
#include <vector>
using uint = unsigned int;
using ZoomLevel = unsigned int;
inline unsigned int to_underlying(ZoomLevel zoom) { return zoom; }
enum class BlitterMode { Normal, ColourRemap, CrashRemap, BlackRemap, Transparent, TransparentRemap };
struct Blitter {
    struct BlitterParams {
        const void *sprite;
        void *dst;
        const uint8_t *remap;
        int top, left, pitch, skip_top, skip_left, height, width, sprite_width;
    };
};
struct Blitter_8bppSimple { void Draw(Blitter::BlitterParams *, BlitterMode, ZoomLevel); };
'''
harness = r'''
int main(int argc, char **argv) {
    assert(argc == 2);
    const std::string directory(argv[1]);
    constexpr int sw = 66, sh = 52, width = 288, height = 232;
    Blitter_8bppSimple blitter;
    for (const std::string name : {"parent", "child"}) {
        std::ifstream input(directory + "/" + name + ".alpha", std::ios::binary);
        assert(input.good());
        std::vector<uint8_t> alpha{std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>()};
        assert(alpha.size() == sw*sh);
        std::vector<uint8_t> root(sw*4*sh*4);
        for (int y = 0; y < sh*4; ++y) for (int x = 0; x < sw*4; ++x) {
            auto a = alpha[(y/4)*sw+x/4]; assert(a == 0 || a == 255);
            root[y*(sw*4)+x] = a ? 1 : 0;
        }
        for (ZoomLevel zoom = 0; zoom < 6; ++zoom) {
            std::vector<uint8_t> pixels(width*height, 0);
            const int dw = UnScaleByZoom(sw*4, zoom), dh = UnScaleByZoom(sh*4, zoom);
            Blitter::BlitterParams bp{root.data(), pixels.data(), nullptr, 8, 8, width, 0, 0, dh, dw, sw*4};
            blitter.Draw(&bp, BlitterMode::Normal, zoom);
            int opaque = 0;
            for (int y = 0; y < height; ++y) for (int x = 0; x < width; ++x) {
                uint8_t expected = 0;
                if (x >= 8 && x < 8+dw && y >= 8 && y < 8+dh) {
                    const int sx = ((x-8) * (1 << zoom)) / 4;
                    const int sy = ((y-8) * (1 << zoom)) / 4;
                    expected = alpha[sy*sw+sx] ? 1 : 0;
                }
                assert(pixels[y*width+x] == expected);
                opaque += pixels[y*width+x] != 0;
            }
            const std::string output = directory + "/native-sampling-" + name + "-" + std::to_string(zoom) + ".bin";
            std::ofstream file(output, std::ios::binary);
            file.write(reinterpret_cast<const char *>(pixels.data()), pixels.size()); assert(file.good());
            std::cout << name << ',' << zoom << ',' << dw << ',' << dh << ',' << opaque << '\n';
        }
    }
}
'''
cpp = work / "native-mask-sampling.cpp"
assert not cpp.exists(), "preserve an earlier run; use a fresh output directory"
cpp.write_text(preamble + "\n".join(helpers) + draw + harness)
subprocess.run(["clang++", "-std=c++20", "-O2", "-Wall", "-Wextra", "-Werror", str(cpp), "-o", str(work / "native-mask-sampling")], check=True)
result = subprocess.run([str(work / "native-mask-sampling"), str(work)], capture_output=True, text=True, check=True)
(work / "native-mask-sampling.csv").write_text("source,zoom_index,width,height,opaque\n" + result.stdout)
(work / "native-mask-sampling.log").write_text(result.stderr)
(work / "native-source.json").write_text(json.dumps({"pin": pin, "source_sha256": hashlib.sha256(blitter.encode()).hexdigest(), "draw_sha256": hashlib.sha256(draw.encode()).hexdigest(), "zoom_helpers_sha256": hashlib.sha256("\n".join(helpers).encode()).hexdigest(), "scope": __doc__}, indent=2) + "\n")
print(result.stdout, end="")
