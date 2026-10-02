#!/usr/bin/env python3
"""Run the original OpenTTD 8bpp Draw on a captured binary-alpha pair.

Inputs: parent.alpha and child.alpha, each 66x52 bytes (only 0 or 255).
Outputs are ownership tags, not the native palette or a SAV raster oracle.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('directory', type=Path)
parser.add_argument('--reference', type=Path, default=Path('reference/openttd-15.3-oracle'))
args = parser.parse_args()
work = args.directory.resolve()
pin = subprocess.check_output(['git', '-C', str(args.reference), 'rev-parse', 'HEAD'], text=True).strip()
assert pin == '14ec60f248547d4d062a1160f0fc26d742319888', 'the native source pin must match the recorded oracle'
source = subprocess.check_output(['git', '-C', str(args.reference), 'show', 'HEAD:src/blitter/8bpp_optimized.cpp'], text=True)
start = source.index('void Blitter_8bppOptimized::Draw(')
end = source.index('\nSprite *Blitter_8bppOptimized::Encode(', start)
draw = source[start:end]
preamble = r'''
#include <algorithm>
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
enum class BlitterMode { Normal, ColourRemap, CrashRemap, BlackRemap, Transparent, TransparentRemap };
struct Blitter {
    struct BlitterParams {
        const void *sprite;
        void *dst;
        const uint8_t *remap;
        int top, left, pitch, skip_top, skip_left, height, width;
    };
};
struct Blitter_8bppOptimized {
    struct SpriteData { uint32_t offset[6]; uint8_t data[]; };
    void Draw(Blitter::BlitterParams *, BlitterMode, ZoomLevel);
};
'''
harness = r'''
std::vector<uint8_t> read(const std::string &path) {
    std::ifstream input(path, std::ios::binary);
    assert(input.good());
    return {std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>()};
}
std::vector<uint8_t> scale(const std::vector<uint8_t> &alpha) {
    assert(alpha.size() == 66*52);
    std::vector<uint8_t> result(132*104);
    for (int y = 0; y < 104; ++y) for (int x = 0; x < 132; ++x) {
        uint8_t a = alpha[(y/2)*66+x/2];
        assert(a == 0 || a == 255);
        result[y*132+x] = a;
    }
    return result;
}
std::vector<uint8_t> encode(const std::vector<uint8_t> &alpha) {
    // One native RLE stream at zoom index zero; scaffold does not replace Draw.
    std::vector<uint8_t> result(6*sizeof(uint32_t), 0);
    for (int y = 0; y < 104; ++y) {
        int x = 0;
        while (x < 132) {
            int trans = 0, pixels = 0;
            while (x < 132 && alpha[y*132+x] == 0) { ++trans; ++x; }
            while (x < 132 && alpha[y*132+x] != 0) { ++pixels; ++x; }
            result.push_back(trans); result.push_back(pixels);
            result.insert(result.end(), pixels, 1);
        }
        result.push_back(0); result.push_back(0);
    }
    return result;
}
void write(const std::string &path, const std::vector<uint8_t> &bytes) {
    std::ofstream output(path, std::ios::binary);
    output.write(reinterpret_cast<const char *>(bytes.data()), bytes.size());
    assert(output.good());
}
int main(int argc, char **argv) {
    assert(argc == 2);
    const std::string directory(argv[1]);
    auto parent = scale(read(directory+"/parent.alpha"));
    auto child = scale(read(directory+"/child.alpha"));
    auto parent_rle = encode(parent), child_rle = encode(child);
    std::array<uint8_t, 256> remap{};
    for (int i = 0; i < 256; ++i) remap[i] = i;
    remap[0] = 3; remap[1] = 2; // Glass on background vs glass on parent.
    Blitter_8bppOptimized blitter;
    std::array<std::vector<uint8_t>, 2> outputs;
    for (int reversed = 0; reversed < 2; ++reversed) {
        auto &pixels = outputs[reversed]; pixels.assign(parent.size(), 0);
        auto apply = [&](const std::vector<uint8_t> &sprite, BlitterMode mode) {
            Blitter::BlitterParams bp{sprite.data(), pixels.data(), remap.data(), 0, 0, 132, 0, 0, 104, 132};
            blitter.Draw(&bp, mode, 0);
        };
        if (!reversed) {
            apply(parent_rle, BlitterMode::Normal); apply(child_rle, BlitterMode::Transparent);
        } else {
            apply(child_rle, BlitterMode::Transparent); apply(parent_rle, BlitterMode::Normal);
        }
        for (size_t i = 0; i < pixels.size(); ++i) {
            uint8_t expected = reversed ? (parent[i] ? 1 : (child[i] ? 3 : 0))
                : (child[i] ? (parent[i] ? 2 : 3) : (parent[i] ? 1 : 0));
            assert(pixels[i] == expected);
        }
        write(directory+(reversed ? "/native-child-parent.bin" : "/native-parent-child.bin"), pixels);
        std::array<int, 4> counts{}; for (uint8_t value : pixels) ++counts[value];
        std::cout << (reversed ? "child_parent" : "parent_child") << ',' << pixels.size();
        for (int count : counts) std::cout << ',' << count;
        std::cout << '\n';
    }
    int differences = 0; for (size_t i = 0; i < parent.size(); ++i) differences += outputs[0][i] != outputs[1][i];
    assert(differences == 68);
    std::cerr << "ordered native tags differ at " << differences << " pixels\n";
}
'''
cpp = work / 'native-mask-pair.cpp'
cpp.write_text(preamble + draw + harness)
subprocess.run(['clang++', '-std=c++20', '-O2', '-Wall', '-Wextra', str(cpp), '-o', str(work/'native-mask-pair')], check=True)
result = subprocess.run([str(work/'native-mask-pair'), str(work)], capture_output=True, text=True, check=True)
(work/'native-mask-pair.csv').write_text('order,pixels,background,parent,glass_on_parent,glass_on_background\n'+result.stdout)
(work/'native-mask-pair.log').write_text(result.stderr)
(work/'native-source.json').write_text(json.dumps({'pin': pin, 'file_sha256': hashlib.sha256(source.encode()).hexdigest(), 'draw_sha256': hashlib.sha256(draw.encode()).hexdigest(), 'draw_bytes': len(draw.encode()), 'scope': 'binary alpha at 2x; ownership tag remap, not native palette/decoder/SAV'}, indent=2)+'\n')
print(result.stdout+result.stderr)
