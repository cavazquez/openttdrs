#!/usr/bin/env bash
# Integra el export de snapshots (#110) en un clon OpenTTD @ pin #109.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PATCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST="${1:-${ROOT}/reference/openttd-upstream}"

# shellcheck source=../../scripts/lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

EXPECTED="$(openttd_manifest_get "$ROOT" commit)"
if [[ ! -e "${DEST}/.git" ]]; then
  echo "error: no hay clon en ${DEST}; corré ./scripts/fetch-openttd-reference.sh" >&2
  exit 1
fi
ACTUAL="$(git -C "${DEST}" rev-parse HEAD)"
MODE="full"
if [[ "${ACTUAL}" != "${EXPECTED}" ]]; then
	if [[ "${OPENTTDRS_ALLOW_UNPINNED:-0}" != "1" ]]; then
		echo "error: ${DEST} está en ${ACTUAL}, manifiesto espera ${EXPECTED}" >&2
		echo "  ./scripts/fetch-openttd-reference.sh" >&2
		echo "  o OPENTTDRS_ALLOW_UNPINNED=1 $0 ${DEST} para usar otro árbol bajo tu responsabilidad" >&2
		exit 1
	fi
	echo "warning: integrando exportador en árbol OpenTTD no pinneado ${ACTUAL}" >&2
	MODE="world_raw_only"
fi

if [[ "${MODE}" == "world_raw_only" ]]; then
	cp "${PATCH_DIR}/src/world_raw_export.cpp" "${DEST}/src/world_raw_export.cpp"
	cp "${PATCH_DIR}/src/world_raw_export.h" "${DEST}/src/world_raw_export.h"
else
	cp "${PATCH_DIR}/src/snapshot_export.cpp" "${DEST}/src/snapshot_export.cpp"
	cp "${PATCH_DIR}/src/snapshot_export.h" "${DEST}/src/snapshot_export.h"
fi
cp "${PATCH_DIR}/src/world_semantic_export.cpp" "${DEST}/src/world_semantic_export.cpp"
cp "${PATCH_DIR}/src/world_semantic_export.h" "${DEST}/src/world_semantic_export.h"
cp "${PATCH_DIR}/src/world_draw_export.cpp" "${DEST}/src/world_draw_export.cpp"
cp "${PATCH_DIR}/src/world_draw_export.h" "${DEST}/src/world_draw_export.h"
cp "${PATCH_DIR}/src/world_screenshot_export.cpp" "${DEST}/src/world_screenshot_export.cpp"
cp "${PATCH_DIR}/src/world_screenshot_export.h" "${DEST}/src/world_screenshot_export.h"

python3 - "$DEST" "$MODE" <<'PY'
from pathlib import Path
import os
import sys

dest = Path(sys.argv[1])
mode = sys.argv[2]
allow_unpinned = os.environ.get("OPENTTDRS_ALLOW_UNPINNED") == "1"


def add_cmake_source(cmake: Path, text: str, source: str) -> str:
    if source in text:
        print(f"CMakeLists: {source} ya listado")
        return text
    if "    console_cmds.cpp\n" not in text:
        raise SystemExit("no encuentro console_cmds.cpp en src/CMakeLists.txt")
    text = text.replace(
        "    console_cmds.cpp\n",
        f"    console_cmds.cpp\n    {source}\n",
        1,
    )
    print(f"CMakeLists: {source}")
    return text


def integrate_world_draw_viewport(dest: Path) -> None:
    """Añade el capturador trace-only al punto central del renderer C++."""
    header = dest / "src" / "viewport_func.h"
    header_text = header.read_text(encoding="utf-8")
    declaration = "bool OpenttdrsCaptureWorldDraw();\n"
    if declaration not in header_text:
        anchor = "void ViewportDoDraw(const Viewport &vp, int left, int top, int right, int bottom);\n"
        if anchor not in header_text:
            raise SystemExit("no encuentro declaración ViewportDoDraw")
        header_text = header_text.replace(anchor, anchor + declaration, 1)
        header.write_text(header_text, encoding="utf-8")
        print("viewport_func: declaración world-draw")

    viewport = dest / "src" / "viewport.cpp"
    text = viewport.read_text(encoding="utf-8")
    if '#include "world_draw_export.h"' not in text:
        anchor = '#include "viewport_func.h"\n'
        if anchor not in text:
            raise SystemExit("no encuentro include viewport_func.h")
        text = text.replace(anchor, anchor + '#include "world_draw_export.h"\n', 1)
    if '#include "world_screenshot_export.h"' not in text:
        anchor = '#include "viewport_func.h"\n'
        if anchor not in text:
            raise SystemExit("no encuentro include viewport_func.h para world-screenshot")
        text = text.replace(anchor, anchor + '#include "world_screenshot_export.h"\n', 1)

    tile_marker = (
        "static void AddTileSpriteToDraw(SpriteID image, PaletteID pal, int32_t x, int32_t y, int z, const SubSprite *sub = nullptr, int extra_offs_x = 0, int extra_offs_y = 0)\n"
        "{\n"
        "\tassert((image & SPRITE_MASK) < MAX_SPRITES);"
    )
    if "OpenttdrsWorldDrawRecordTileSprite" not in text:
        if tile_marker not in text:
            raise SystemExit("no encuentro AddTileSpriteToDraw")
        replacement = (
            "static void AddTileSpriteToDraw(SpriteID image, PaletteID pal, int32_t x, int32_t y, int z, const SubSprite *sub = nullptr, int extra_offs_x = 0, int extra_offs_y = 0)\n"
            "{\n"
            "\tif (OpenttdrsWorldDrawCaptureActive()) {\n"
            "\t\tOpenttdrsWorldDrawRecordTileSprite(image, pal, x, y, z);\n"
            "\t\treturn;\n"
            "\t}\n"
            "\tassert((image & SPRITE_MASK) < MAX_SPRITES);"
        )
        text = text.replace(tile_marker, replacement, 1)

    sortable_marker = (
        "void AddSortableSpriteToDraw(SpriteID image, PaletteID pal, int x, int y, int z, const SpriteBounds &bounds, bool transparent, const SubSprite *sub)\n"
        "{\n"
        "\tint32_t left, right, top, bottom;"
    )
    if "OpenttdrsWorldDrawRecordSortable" not in text:
        if sortable_marker not in text:
            raise SystemExit("no encuentro AddSortableSpriteToDraw")
        replacement = (
            "void AddSortableSpriteToDraw(SpriteID image, PaletteID pal, int x, int y, int z, const SpriteBounds &bounds, bool transparent, const SubSprite *sub)\n"
            "{\n"
            "\tif (OpenttdrsWorldDrawCaptureActive()) {\n"
            "\t\tconst auto combine_mode = static_cast<uint8_t>(_vd.combine_sprites);\n"
            "\t\tOpenttdrsWorldDrawRecordSortable(image, pal, x, y, z,\n"
            "\t\t\tbounds.origin.x, bounds.origin.y, bounds.origin.z,\n"
            "\t\t\tbounds.extent.x, bounds.extent.y, bounds.extent.z,\n"
            "\t\t\tbounds.offset.x, bounds.offset.y, bounds.offset.z, transparent, combine_mode);\n"
            "\t\tif (_vd.combine_sprites != SPRITE_COMBINE_ACTIVE) {\n"
            "\t\t\t/* Mantener el mínimo estado que necesitan foundations y children; no rasterizamos. */\n"
            "\t\t\t_vd.parent_sprites_to_draw.emplace_back();\n"
            "\t\t\t_vd.last_child = LAST_CHILD_PARENT;\n"
            "\t\t\tif (_vd.combine_sprites == SPRITE_COMBINE_PENDING) _vd.combine_sprites = SPRITE_COMBINE_ACTIVE;\n"
            "\t\t}\n"
            "\t\treturn;\n"
            "\t}\n"
            "\tint32_t left, right, top, bottom;"
        )
        text = text.replace(sortable_marker, replacement, 1)

    child_marker = (
        "void AddChildSpriteScreen(SpriteID image, PaletteID pal, int x, int y, bool transparent, const SubSprite *sub, bool scale, bool relative)\n"
        "{\n"
        "\tassert((image & SPRITE_MASK) < MAX_SPRITES);"
    )
    if "OpenttdrsWorldDrawRecordChild" not in text:
        if child_marker not in text:
            raise SystemExit("no encuentro AddChildSpriteScreen")
        replacement = (
            "void AddChildSpriteScreen(SpriteID image, PaletteID pal, int x, int y, bool transparent, const SubSprite *sub, bool scale, bool relative)\n"
            "{\n"
            "\tif (OpenttdrsWorldDrawCaptureActive()) {\n"
            "\t\tOpenttdrsWorldDrawRecordChild(image, pal, x, y, transparent, scale, relative);\n"
            "\t\treturn;\n"
            "\t}\n"
            "\tassert((image & SPRITE_MASK) < MAX_SPRITES);"
        )
        text = text.replace(child_marker, replacement, 1)

    vehicle_marker = "\tViewportAddVehicles(&_vd.dpi);\n"
    if "OpenttdrsWorldScreenshotHideVehicles" not in text:
        if vehicle_marker not in text:
            raise SystemExit("no encuentro ViewportAddVehicles para la captura limpia")
        text = text.replace(
            vehicle_marker,
            "\tif (!OpenttdrsWorldScreenshotHideVehicles()) ViewportAddVehicles(&_vd.dpi);\n",
            1,
        )

    start_marker = "void StartSpriteCombine()\n{\n\tassert(_vd.combine_sprites == SPRITE_COMBINE_NONE);"
    if "OpenttdrsWorldDrawRecordCombineStart" not in text:
        if start_marker not in text:
            raise SystemExit("no encuentro StartSpriteCombine")
        text = text.replace(
            start_marker,
            "void StartSpriteCombine()\n{\n\tif (OpenttdrsWorldDrawCaptureActive()) OpenttdrsWorldDrawRecordCombineStart();\n\tassert(_vd.combine_sprites == SPRITE_COMBINE_NONE);",
            1,
        )

    end_marker = "void EndSpriteCombine()\n{\n\tassert(_vd.combine_sprites != SPRITE_COMBINE_NONE);"
    if "OpenttdrsWorldDrawRecordCombineEnd" not in text:
        if end_marker not in text:
            raise SystemExit("no encuentro EndSpriteCombine")
        text = text.replace(
            end_marker,
            "void EndSpriteCombine()\n{\n\tif (OpenttdrsWorldDrawCaptureActive()) OpenttdrsWorldDrawRecordCombineEnd();\n\tassert(_vd.combine_sprites != SPRITE_COMBINE_NONE);",
            1,
        )

    anchor = "\n/**\n * Add a string to draw in the current viewport."
    if anchor not in text:
        raise SystemExit("no encuentro ancla posterior a ViewportAddLandscape")
    capture = r'''

/**
 * Ejecuta los `draw_tile_proc` reales sin framebuffer ni clipping. Está
 * pensado para el exportador de paridad: no incluye vehículos, labels ni UI.
 */
bool OpenttdrsCaptureWorldDraw()
{
	OpenttdrsWorldDrawBounds bounds;
	if (!OpenttdrsWorldDrawCaptureBounds(bounds)) return true;

	_vd = {};
	/* Algunos draw procs (por ejemplo carretera) consultan `_cur_dpi->zoom`.
	 * El servidor dedicado no tiene framebuffer, pero para la selección de
	 * sprites basta un viewport lógico enorme, sin clipping. */
	_vd.dpi.left = -1000000000;
	_vd.dpi.top = -1000000000;
	_vd.dpi.width = 2000000000;
	_vd.dpi.height = 2000000000;
	_vd.dpi.zoom = ZoomLevel::Normal;
	AutoRestoreBackup dpi_backup(_cur_dpi, &_vd.dpi);
	_vd.combine_sprites = SPRITE_COMBINE_NONE;
	_vd.last_child = LAST_CHILD_NONE;
	for (uint32_t y = bounds.begin_y; y < bounds.end_y; y++) {
		for (uint32_t x = bounds.begin_x; x < bounds.end_x; x++) {
			_cur_ti.tile = TileXY(x, y);
			_cur_ti.x = static_cast<int>(x * TILE_SIZE);
			_cur_ti.y = static_cast<int>(y * TILE_SIZE);
			std::tie(_cur_ti.tileh, _cur_ti.z) = GetTilePixelSlope(_cur_ti.tile);
			auto [raw_tileh, raw_z] = GetTileSlopeZ(_cur_ti.tile);
			auto [foundation_tileh, foundation_z] = GetFoundationSlope(_cur_ti.tile);
			_vd.foundation_part = FOUNDATION_PART_NONE;
			_vd.foundation[0] = -1;
			_vd.foundation[1] = -1;
			_vd.last_foundation_child[0] = LAST_CHILD_NONE;
			_vd.last_foundation_child[1] = LAST_CHILD_NONE;
			OpenttdrsWorldDrawBeginTile(x, y, static_cast<uint8_t>(GetTileType(_cur_ti.tile)),
				static_cast<uint8_t>(raw_tileh), static_cast<uint32_t>(raw_z),
				static_cast<uint8_t>(foundation_tileh), static_cast<uint32_t>(foundation_z));
			_tile_type_procs[GetTileType(_cur_ti.tile)]->draw_tile_proc(&_cur_ti);
			OpenttdrsWorldDrawEndTile();
		}
	}

	_vd.tile_sprites_to_draw.clear();
	_vd.parent_sprites_to_draw.clear();
	_vd.parent_sprites_to_sort.clear();
	_vd.child_screen_sprites_to_draw.clear();
	return OpenttdrsFinishWorldDraw();
}
'''
    capture_marker = "\n/**\n * Ejecuta los `draw_tile_proc` reales sin framebuffer ni clipping."
    if capture_marker in text:
        # Reemplazar nuestra inyección completa al reintegrar: de ese modo un
        # árbol OpenTTD ya parcheado recibe también correcciones del oráculo.
        start = text.index(capture_marker)
        end = text.index(anchor, start)
        text = text[:start] + capture + text[end:]
    elif "bool OpenttdrsCaptureWorldDraw()" not in text:
        text = text.replace(anchor, capture + anchor, 1)
    else:
        raise SystemExit("ya existe OpenttdrsCaptureWorldDraw ajeno al integrador")

    viewport.write_text(text, encoding="utf-8")
    print("viewport: capturador world-draw trace-only")


def integrate_world_draw_foundation(dest: Path) -> None:
    """Traza el bloque de paredes que `DrawFoundation` eligió realmente."""
    landscape = dest / "src" / "landscape.cpp"
    text = landscape.read_text(encoding="utf-8")
    if '#include "world_draw_export.h"' not in text:
        anchor = '#include "landscape.h"\n'
        if anchor not in text:
            raise SystemExit("no encuentro include landscape.h")
        text = text.replace(anchor, anchor + '#include "world_draw_export.h"\n', 1)

    draw_start = text.find("void DrawFoundation(TileInfo *ti, Foundation f)\n")
    if draw_start < 0:
        raise SystemExit("no encuentro DrawFoundation")
    prelude_start = text.find("\tuint sprite_block = 0;\n", draw_start)
    prelude_end = text.find("\n\n\t/* Use the original slope sprites", prelude_start)
    if prelude_start < 0 or prelude_end < 0:
        raise SystemExit("no encuentro selección de bloque DrawFoundation")
    replacement = (
        "\tuint sprite_block = 0;\n"
        "\tauto [slope, z] = GetFoundationPixelSlope(ti->tile);\n"
        "\tconst bool has_nw = HasFoundationNW(ti->tile, slope, z);\n"
        "\tconst bool has_ne = HasFoundationNE(ti->tile, slope, z);\n"
        "\n"
        "\t/* Las mismas ocho alturas que usan HasFoundationNW/NE. La traza\n"
        "\t * conserva los valores antes de reducirlos al booleano para que el\n"
        "\t * cliente pueda contrastar orientación y fundamento de cada vecino. */\n"
        "\tint nw_w_here = z;\n"
        "\tint nw_n_here = z;\n"
        "\tGetSlopePixelZOnEdge(slope, DIAGDIR_NW, nw_w_here, nw_n_here);\n"
        "\tauto [nw_slope, nw_z] = GetFoundationPixelSlope(TileAddXY(ti->tile, 0, -1));\n"
        "\tint nw_w_neighbour = nw_z;\n"
        "\tint nw_n_neighbour = nw_z;\n"
        "\tGetSlopePixelZOnEdge(nw_slope, DIAGDIR_SE, nw_w_neighbour, nw_n_neighbour);\n"
        "\tint ne_e_here = z;\n"
        "\tint ne_n_here = z;\n"
        "\tGetSlopePixelZOnEdge(slope, DIAGDIR_NE, ne_e_here, ne_n_here);\n"
        "\tauto [ne_slope, ne_z] = GetFoundationPixelSlope(TileAddXY(ti->tile, -1, 0));\n"
        "\tint ne_e_neighbour = ne_z;\n"
        "\tint ne_n_neighbour = ne_z;\n"
        "\tGetSlopePixelZOnEdge(ne_slope, DIAGDIR_SW, ne_e_neighbour, ne_n_neighbour);\n"
        "\n"
        "\t/* Select the needed block of foundations sprites\n"
        "\t * Block 0: Walls at NW and NE edge\n"
        "\t * Block 1: Wall  at        NE edge\n"
        "\t * Block 2: Wall  at NW        edge\n"
        "\t * Block 3: No walls at NW or NE edge\n"
        "\t */\n"
        "\tif (!has_nw) sprite_block += 1;\n"
        "\tif (!has_ne) sprite_block += 2;\n"
        "\tOpenttdrsWorldDrawRecordFoundation(\n"
        "\t\tstatic_cast<uint8_t>(f), static_cast<uint8_t>(slope),\n"
        "\t\tstatic_cast<uint32_t>(z / TILE_HEIGHT), static_cast<uint8_t>(sprite_block), has_nw, has_ne,\n"
        "\t\tnw_w_here, nw_n_here, nw_w_neighbour, nw_n_neighbour,\n"
        "\t\tne_e_here, ne_n_here, ne_e_neighbour, ne_n_neighbour\n"
        "\t);"
    )
    text = text[:prelude_start] + replacement + text[prelude_end:]
    landscape.write_text(text, encoding="utf-8")
    print("landscape: traza de decisión DrawFoundation")


def integrate_headless_raster_blitter(dest: Path) -> None:
    """Permite que un build dedicado rasterice sólo para el oráculo PNG.

    OpenTTD excluye todos los blitters reales cuando OPTION_DEDICATED=ON y
    deja únicamente `null`, cuya profundidad es cero. El driver dedicado sí
    tiene un framebuffer en memoria; sumamos los blitters simples 8bpp y
    32bpp bajo una opción explícita para que el capturador use el pipeline
    oficial sin SDL ni ventana.
    """
    cmake = dest / "src" / "blitter" / "CMakeLists.txt"
    text = cmake.read_text(encoding="utf-8")
    marker = "# openttdrs headless raster oracle\n"
    headless_32bpp = marker + "add_files(\n    32bpp_base.cpp\n"
    if marker not in text:
        anchor = "add_files(\n    base.hpp\n"
        if anchor not in text:
            raise SystemExit("no encuentro bloque base de src/blitter/CMakeLists.txt")
        block = (
            "# openttdrs headless raster oracle\n"
            "add_files(\n"
            "    32bpp_base.cpp\n"
            "    32bpp_base.hpp\n"
            "    32bpp_simple.cpp\n"
            "    32bpp_simple.hpp\n"
            "    8bpp_base.cpp\n"
            "    8bpp_base.hpp\n"
            "    8bpp_simple.cpp\n"
            "    8bpp_simple.hpp\n"
            "    CONDITION OPTION_DEDICATED AND OPENTTDRS_HEADLESS_RASTER\n"
            ")\n\n"
        )
        text = text.replace(anchor, block + anchor, 1)
        cmake.write_text(text, encoding="utf-8")
        print("blitter: 8bpp/32bpp-simple opcionales para oráculo headless")
    elif headless_32bpp not in text:
        anchor = marker + "add_files(\n"
        if anchor not in text:
            raise SystemExit("no encuentro bloque headless de src/blitter/CMakeLists.txt")
        text = text.replace(
            anchor,
            anchor
            + "    32bpp_base.cpp\n"
            + "    32bpp_base.hpp\n"
            + "    32bpp_simple.cpp\n"
            + "    32bpp_simple.hpp\n",
            1,
        )
        cmake.write_text(text, encoding="utf-8")
        print("blitter: se amplió el oráculo headless a 32bpp-simple")
    else:
        print("blitter: oráculo headless ya listado")

if mode == "world_raw_only":
    cmake = dest / "src" / "CMakeLists.txt"
    text = cmake.read_text(encoding="utf-8")
    text = text.replace("    snapshot_export.cpp\n", "", 1)
    if "world_raw_export.cpp" not in text:
        if "console_cmds.cpp" not in text:
            raise SystemExit("no encuentro console_cmds.cpp en src/CMakeLists.txt")
        text = text.replace(
            "    console_cmds.cpp\n",
            "    console_cmds.cpp\n    world_raw_export.cpp\n",
            1,
        )
        print("CMakeLists: world_raw_export.cpp")
    else:
        print("CMakeLists: world_raw_export.cpp ya listado")
    if "world_semantic_export.cpp" not in text:
        if "console_cmds.cpp" not in text:
            raise SystemExit("no encuentro console_cmds.cpp en src/CMakeLists.txt")
        text = text.replace(
            "    console_cmds.cpp\n",
            "    console_cmds.cpp\n    world_semantic_export.cpp\n",
            1,
        )
        print("CMakeLists: world_semantic_export.cpp")
    else:
        print("CMakeLists: world_semantic_export.cpp ya listado")
    text = add_cmake_source(cmake, text, "world_draw_export.cpp")
    text = add_cmake_source(cmake, text, "world_screenshot_export.cpp")
    cmake.write_text(text, encoding="utf-8")

    after = dest / "src" / "saveload" / "afterload.cpp"
    at = after.read_text(encoding="utf-8")
    at = at.replace('#include "../snapshot_export.h"\n', "", 1)
    if '#include "../world_raw_export.h"' not in at:
        nl = at.find("\n#include ")
        if nl < 0:
            raise SystemExit("no encuentro includes en afterload.cpp")
        at = at[: nl + 1] + '#include "../world_raw_export.h"\n' + at[nl + 1 :]
        print("afterload: include world-raw")
    if '#include "../world_semantic_export.h"' not in at:
        nl = at.find("\n#include ")
        if nl < 0:
            raise SystemExit("no encuentro includes en afterload.cpp")
        at = at[: nl + 1] + '#include "../world_semantic_export.h"\n' + at[nl + 1 :]
        print("afterload: include world-semantic")
    if '#include "../world_draw_export.h"' not in at:
        nl = at.find("\n#include ")
        if nl < 0:
            raise SystemExit("no encuentro includes en afterload.cpp")
        at = at[: nl + 1] + '#include "../world_draw_export.h"\n' + at[nl + 1 :]
        print("afterload: include world-draw")
    if '#include "../world_screenshot_export.h"' not in at:
        nl = at.find("\n#include ")
        if nl < 0:
            raise SystemExit("no encuentro includes en afterload.cpp")
        at = at[: nl + 1] + '#include "../world_screenshot_export.h"\n' + at[nl + 1 :]
        print("afterload: include world-screenshot")

    snapshot_hook = (
        "\tif (!OpenttdrsMaybeExportSnapshot(\"\")) {\n"
        "\t\tDebug(misc, 0, \"openttdrs snapshot export failed\");\n"
        "\t}\n"
    )
    pbs_hook = "\tOpenttdrsMaybeStartPbsTrace(\"\");\n"
    fta_hook = "\tOpenttdrsMaybeStartAirportFtaTrace(\"\");\n"
    raw_hook = (
        "\tif (!OpenttdrsMaybeExportWorldRaw(\"\")) {\n"
        "\t\tDebug(misc, 0, \"openttdrs world-raw export failed\");\n"
        "\t}\n"
    )
    semantic_hook = (
        "\tif (!OpenttdrsMaybeExportWorldSemantic(\"\")) {\n"
        "\t\tDebug(misc, 0, \"openttdrs world-semantic export failed\");\n"
        "\t}\n"
    )
    draw_hook = (
        "\tif (!OpenttdrsMaybeStartWorldDraw(\"\")) {\n"
        "\t\tDebug(misc, 0, \"openttdrs world-draw export failed to start\");\n"
        "\t} else if (OpenttdrsWorldDrawCaptureActive() && !OpenttdrsCaptureWorldDraw()) {\n"
        "\t\tDebug(misc, 0, \"openttdrs world-draw capture failed\");\n"
        "\t}\n"
    )
    screenshot_hook = (
        "\tif (!OpenttdrsMaybeCaptureWorldScreenshot()) {\n"
        "\t\tDebug(misc, 0, \"openttdrs world-screenshot capture failed\");\n"
        "\t}\n"
    )
    for hook in (snapshot_hook, pbs_hook, fta_hook, raw_hook, semantic_hook, draw_hook, screenshot_hook):
        at = at.replace(hook, "", 1)
    anchor = "\treturn true;\n}\n\n/**\n * Reload all NewGRF"
    if anchor not in at:
        raise SystemExit("no encuentro ancla return true de AfterLoadGame")
    at = at.replace(anchor, raw_hook + semantic_hook + draw_hook + screenshot_hook + "\treturn true;\n}\n\n/**\n * Reload all NewGRF", 1)
    after.write_text(at, encoding="utf-8")
    integrate_world_draw_viewport(dest)
    integrate_world_draw_foundation(dest)
    integrate_headless_raster_blitter(dest)
    print("afterload: hooks world-raw/world-semantic/world-draw/world-screenshot AfterLoadGame")

    openttd = dest / "src" / "openttd.cpp"
    ot = openttd.read_text(encoding="utf-8")
    cleaned = ot.replace('#include "snapshot_export.h"\n', "", 1)
    if cleaned != ot:
        openttd.write_text(cleaned, encoding="utf-8")
        print("openttd: se retiró include snapshot incompatible")
    print("Integración minimal world-raw/world-semantic/world-draw lista (PBS/FTA/snapshot no se compilan en árbol no pinneado)")
    raise SystemExit(0)

cmake = dest / "src" / "CMakeLists.txt"
text = cmake.read_text(encoding="utf-8")
if "snapshot_export.cpp" not in text:
    if "console_cmds.cpp" not in text:
        raise SystemExit("no encuentro console_cmds.cpp en src/CMakeLists.txt")
    text = text.replace(
        "    console_cmds.cpp\n",
        "    console_cmds.cpp\n    snapshot_export.cpp\n",
        1,
    )
    cmake.write_text(text, encoding="utf-8")
    print("CMakeLists: snapshot_export.cpp")
else:
    print("CMakeLists: ya listado")
if "world_semantic_export.cpp" not in text:
    if "console_cmds.cpp" not in text:
        raise SystemExit("no encuentro console_cmds.cpp en src/CMakeLists.txt")
    text = text.replace(
        "    console_cmds.cpp\n",
        "    console_cmds.cpp\n    world_semantic_export.cpp\n",
        1,
    )
    cmake.write_text(text, encoding="utf-8")
    print("CMakeLists: world_semantic_export.cpp")
else:
    print("CMakeLists: world_semantic_export.cpp ya listado")
text = add_cmake_source(cmake, text, "world_draw_export.cpp")
text = add_cmake_source(cmake, text, "world_screenshot_export.cpp")
cmake.write_text(text, encoding="utf-8")

after = dest / "src" / "saveload" / "afterload.cpp"
at = after.read_text(encoding="utf-8")
if '#include "../snapshot_export.h"' not in at:
    # Tras el primer bloque de includes del archivo.
    nl = at.find("\n#include ")
    if nl < 0:
        raise SystemExit("no encuentro includes en afterload.cpp")
    at = at[: nl + 1] + '#include "../snapshot_export.h"\n' + at[nl + 1 :]
    print("afterload: include")
if '#include "../world_semantic_export.h"' not in at:
    nl = at.find("\n#include ")
    if nl < 0:
        raise SystemExit("no encuentro includes en afterload.cpp")
    at = at[: nl + 1] + '#include "../world_semantic_export.h"\n' + at[nl + 1 :]
    print("afterload: include world-semantic")
if '#include "../world_draw_export.h"' not in at:
    nl = at.find("\n#include ")
    if nl < 0:
        raise SystemExit("no encuentro includes en afterload.cpp")
    at = at[: nl + 1] + '#include "../world_draw_export.h"\n' + at[nl + 1 :]
    print("afterload: include world-draw")
if '#include "../world_screenshot_export.h"' not in at:
    nl = at.find("\n#include ")
    if nl < 0:
        raise SystemExit("no encuentro includes en afterload.cpp")
    at = at[: nl + 1] + '#include "../world_screenshot_export.h"\n' + at[nl + 1 :]
    print("afterload: include world-screenshot")

hook = (
    "\tif (!OpenttdrsMaybeExportSnapshot(\"\")) {\n"
    "\t\tDebug(misc, 0, \"openttdrs snapshot export failed\");\n"
    "\t}\n"
    "\tOpenttdrsMaybeStartPbsTrace(\"\");\n"
    "\tOpenttdrsMaybeStartAirportFtaTrace(\"\");\n"
)
anchor = "\treturn true;\n}\n\n/**\n * Reload all NewGRF"
if "OpenttdrsMaybeExportSnapshot" not in at:
    if anchor not in at:
        raise SystemExit("no encuentro ancla return true de AfterLoadGame")
    at = at.replace(anchor, hook + "\treturn true;\n}\n\n/**\n * Reload all NewGRF", 1)
    print("afterload: hook AfterLoadGame")
else:
    print("afterload: hook ya presente")

raw_hook = (
    "\tif (!OpenttdrsMaybeExportWorldRaw(\"\")) {\n"
    "\t\tDebug(misc, 0, \"openttdrs world-raw export failed\");\n"
    "\t}\n"
)
semantic_hook = (
    "\tif (!OpenttdrsMaybeExportWorldSemantic(\"\")) {\n"
    "\t\tDebug(misc, 0, \"openttdrs world-semantic export failed\");\n"
    "\t}\n"
)
draw_hook = (
    "\tif (!OpenttdrsMaybeStartWorldDraw(\"\")) {\n"
    "\t\tDebug(misc, 0, \"openttdrs world-draw export failed to start\");\n"
    "\t} else if (OpenttdrsWorldDrawCaptureActive() && !OpenttdrsCaptureWorldDraw()) {\n"
    "\t\tDebug(misc, 0, \"openttdrs world-draw capture failed\");\n"
    "\t}\n"
)
screenshot_hook = (
    "\tif (!OpenttdrsMaybeCaptureWorldScreenshot()) {\n"
    "\t\tDebug(misc, 0, \"openttdrs world-screenshot capture failed\");\n"
    "\t}\n"
)
if "OpenttdrsMaybeExportWorldRaw" not in at:
    if hook not in at:
        raise SystemExit("no encuentro hook snapshot para enganchar world-raw")
    at = at.replace(hook, hook + raw_hook, 1)
    print("afterload: hook world-raw AfterLoadGame")
else:
    print("afterload: hook world-raw ya presente")
if "OpenttdrsMaybeExportWorldSemantic" not in at:
    if raw_hook not in at:
        raise SystemExit("no encuentro hook world-raw para enganchar world-semantic")
    at = at.replace(raw_hook, raw_hook + semantic_hook, 1)
    print("afterload: hook world-semantic AfterLoadGame")
else:
    print("afterload: hook world-semantic ya presente")
if "OpenttdrsMaybeStartWorldDraw" not in at:
    if semantic_hook not in at:
        raise SystemExit("no encuentro hook world-semantic para enganchar world-draw")
    at = at.replace(semantic_hook, semantic_hook + draw_hook, 1)
    print("afterload: hook world-draw AfterLoadGame")
else:
    print("afterload: hook world-draw ya presente")
if "OpenttdrsMaybeCaptureWorldScreenshot" not in at:
    if draw_hook not in at:
        raise SystemExit("no encuentro hook world-draw para enganchar world-screenshot")
    at = at.replace(draw_hook, draw_hook + screenshot_hook, 1)
    print("afterload: hook world-screenshot AfterLoadGame")
else:
    print("afterload: hook world-screenshot ya presente")

after.write_text(at, encoding="utf-8")
integrate_world_draw_viewport(dest)
integrate_world_draw_foundation(dest)
integrate_headless_raster_blitter(dest)

openttd = dest / "src" / "openttd.cpp"
ot = openttd.read_text(encoding="utf-8")
if '#include "snapshot_export.h"' not in ot:
    nl = ot.find("\n#include ")
    if nl < 0:
        raise SystemExit("no encuentro includes en openttd.cpp")
    ot = ot[: nl + 1] + '#include "snapshot_export.h"\n' + ot[nl + 1 :]
    print("openttd: include")

tick_hook = "\tOpenttdrsMaybeExportPbsTraceTick();\n"
tick_anchor = "\tcur_company.Restore();\n"
tick_hooks_available = "OpenttdrsMaybeExportPbsTraceTick" in ot
if not tick_hooks_available:
    if tick_anchor in ot:
        ot = ot.replace(tick_anchor, tick_anchor + tick_hook, 1)
        tick_hooks_available = True
        print("openttd: hook post StateGameLoop")
    elif allow_unpinned:
        print("openttd: ancla PBS cambió; se omiten trazas PBS/FTA en árbol no pinneado")
    else:
        raise SystemExit("no encuentro ancla cur_company.Restore de StateGameLoop")
else:
    print("openttd: hook PBS ya presente")

fta_tick_hook = "\tOpenttdrsMaybeExportAirportFtaTraceTick();\n"
if tick_hooks_available and "OpenttdrsMaybeExportAirportFtaTraceTick" not in ot:
    if "OpenttdrsMaybeExportPbsTraceTick();" in ot:
        ot = ot.replace(
            "\tOpenttdrsMaybeExportPbsTraceTick();\n",
            "\tOpenttdrsMaybeExportPbsTraceTick();\n" + fta_tick_hook,
            1,
        )
        print("openttd: hook airport FTA post StateGameLoop")
    else:
        raise SystemExit("no encuentro hook PBS para enganchar FTA")
elif tick_hooks_available:
    print("openttd: hook FTA ya presente")

if tick_hooks_available and "OpenttdrsMaybeStartAirportFtaTrace" not in at:
    if "OpenttdrsMaybeStartPbsTrace(\"\");" in at:
        at = at.replace(
            "\tOpenttdrsMaybeStartPbsTrace(\"\");\n",
            "\tOpenttdrsMaybeStartPbsTrace(\"\");\n\tOpenttdrsMaybeStartAirportFtaTrace(\"\");\n",
            1,
        )
        after.write_text(at, encoding="utf-8")
        print("afterload: hook Airport FTA")
    else:
        raise SystemExit("no encuentro StartPbsTrace para enganchar FTA")
elif tick_hooks_available:
    print("afterload: hook FTA ya presente")
elif "\tOpenttdrsMaybeStartAirportFtaTrace(\"\");\n" in at:
    # Sin hook post-tick la traza FTA quedaría armada para siempre; raw/snapshot
    # no dependen de ella y siguen siendo válidos en árboles no pinneados.
    at = at.replace("\tOpenttdrsMaybeStartAirportFtaTrace(\"\");\n", "", 1)
    after.write_text(at, encoding="utf-8")
    print("afterload: se omite FTA sin hook post-tick")

openttd.write_text(ot, encoding="utf-8")
PY

echo "Integrado en ${DEST}"
echo "Build dedicated (ejemplo):"
echo "  cmake -B ${DEST}/build -S ${DEST} -DOPTION_DEDICATED=ON -DOPENTTDRS_HEADLESS_RASTER=ON && cmake --build ${DEST}/build -j"
echo "Export:"
echo "  OPENTTDRS_SNAPSHOT_OUT=/tmp/openttd.json OPENTTDRS_OPENTTD_COMMIT=${EXPECTED} \\"
echo "    ${DEST}/build/openttd -D -g path/to/game.sav"
echo "PBS JSONL (post-tick, termina tras N filas):"
echo "  OPENTTDRS_PBS_TRACE_OUT=/tmp/openttd-pbs.jsonl OPENTTDRS_PBS_TRACE_TICKS=40 \\"
echo "    ${DEST}/build/openttd -D -g path/to/game.sav"
openttd_manifest_summary "$ROOT"
