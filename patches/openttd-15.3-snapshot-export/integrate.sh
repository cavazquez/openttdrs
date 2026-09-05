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

python3 - "$DEST" "$MODE" "$PATCH_DIR" <<'PY'
from pathlib import Path
import os
import sys

dest = Path(sys.argv[1])
mode = sys.argv[2]
patch_dir = Path(sys.argv[3])
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

    sorter_forward = "static void ViewportSortParentSprites(ParentSpriteToSortVector *psdv);\n"
    if sorter_forward not in text:
        anchor = "static VpSpriteSorter _vp_sprite_sorter = nullptr;\n"
        if anchor not in text:
            raise SystemExit("no encuentro _vp_sprite_sorter")
        text = text.replace(anchor, anchor + sorter_forward, 1)

    tile_marker = (
        "static void AddTileSpriteToDraw(SpriteID image, PaletteID pal, int32_t x, int32_t y, int z, const SubSprite *sub = nullptr, int extra_offs_x = 0, int extra_offs_y = 0)\n"
        "{\n"
        "\tassert((image & SPRITE_MASK) < MAX_SPRITES);"
    )
    tile_trace_call = "\t\tOpenttdrsWorldDrawRecordTileSprite(image, pal, x, y, z, extra_offs_x, extra_offs_y);\n"
    old_tile_trace_call = "\t\tOpenttdrsWorldDrawRecordTileSprite(image, pal, x, y, z);\n"
    if old_tile_trace_call in text:
        # Migra un árbol ya integrado: las versiones anteriores del oráculo
        # descartaban los offsets que `TILE_SEQ_GROUND` entrega al suelo.
        text = text.replace(old_tile_trace_call, tile_trace_call, 1)
        print("viewport: offsets ground del world-draw actualizados")
    elif "OpenttdrsWorldDrawRecordTileSprite" not in text:
        if tile_marker not in text:
            raise SystemExit("no encuentro AddTileSpriteToDraw")
        replacement = (
            "static void AddTileSpriteToDraw(SpriteID image, PaletteID pal, int32_t x, int32_t y, int z, const SubSprite *sub = nullptr, int extra_offs_x = 0, int extra_offs_y = 0)\n"
            "{\n"
            "\tif (OpenttdrsWorldDrawCaptureActive()) {\n"
            f"{tile_trace_call}"
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
    sortable_legacy_tail = (
        "\t\tif (_vd.combine_sprites != SPRITE_COMBINE_ACTIVE) {\n"
        "\t\t\t/* Mantener el mínimo estado que necesitan foundations y children; no rasterizamos. */\n"
        "\t\t\t_vd.parent_sprites_to_draw.emplace_back();\n"
        "\t\t\t_vd.last_child = LAST_CHILD_PARENT;\n"
        "\t\t\tif (_vd.combine_sprites == SPRITE_COMBINE_PENDING) _vd.combine_sprites = SPRITE_COMBINE_ACTIVE;\n"
        "\t\t}\n"
        "\t\treturn;\n"
    )
    sortable_final_tail = (
        "\t\tif (!OpenttdrsWorldDrawFinalSortRequested()) {\n"
        "\t\t\tif (_vd.combine_sprites != SPRITE_COMBINE_ACTIVE) {\n"
        "\t\t\t\t/* Mantener el mínimo estado que necesitan foundations y children; no rasterizamos. */\n"
        "\t\t\t\t_vd.parent_sprites_to_draw.emplace_back();\n"
        "\t\t\t\t_vd.last_child = LAST_CHILD_PARENT;\n"
        "\t\t\t\tif (_vd.combine_sprites == SPRITE_COMBINE_PENDING) _vd.combine_sprites = SPRITE_COMBINE_ACTIVE;\n"
        "\t\t\t}\n"
        "\t\t\treturn;\n"
        "\t\t}\n"
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
            + sortable_final_tail
            + "\t}\n"
            + "\tint32_t left, right, top, bottom;"
        )
        text = text.replace(sortable_marker, replacement, 1)
    elif "OpenttdrsWorldDrawFinalSortRequested" not in text:
        if sortable_legacy_tail not in text:
            raise SystemExit("no encuentro cola legacy de AddSortableSpriteToDraw")
        text = text.replace(sortable_legacy_tail, sortable_final_tail, 1)

    child_marker = (
        "void AddChildSpriteScreen(SpriteID image, PaletteID pal, int x, int y, bool transparent, const SubSprite *sub, bool scale, bool relative)\n"
        "{\n"
        "\tassert((image & SPRITE_MASK) < MAX_SPRITES);"
    )
    child_legacy_block = (
        "\tif (OpenttdrsWorldDrawCaptureActive()) {\n"
        "\t\tOpenttdrsWorldDrawRecordChild(image, pal, x, y, transparent, scale, relative);\n"
        "\t\treturn;\n"
        "\t}\n"
    )
    child_final_block = (
        "\tif (OpenttdrsWorldDrawCaptureActive()) {\n"
        "\t\tOpenttdrsWorldDrawRecordChild(image, pal, x, y, transparent, scale, relative);\n"
        "\t\tif (!OpenttdrsWorldDrawFinalSortRequested()) return;\n"
        "\t}\n"
    )
    if "OpenttdrsWorldDrawRecordChild" not in text:
        if child_marker not in text:
            raise SystemExit("no encuentro AddChildSpriteScreen")
        replacement = (
            "void AddChildSpriteScreen(SpriteID image, PaletteID pal, int x, int y, bool transparent, const SubSprite *sub, bool scale, bool relative)\n"
            "{\n"
            + child_final_block
            + "\tassert((image & SPRITE_MASK) < MAX_SPRITES);"
        )
        text = text.replace(child_marker, replacement, 1)
    elif "OpenttdrsWorldDrawFinalSortRequested" not in text:
        if child_legacy_block not in text:
            raise SystemExit("no encuentro bloque legacy de AddChildSpriteScreen")
        text = text.replace(child_legacy_block, child_final_block, 1)

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

	if (OpenttdrsWorldDrawFinalSortRequested()) {
		for (auto &parent : _vd.parent_sprites_to_draw) {
			_vd.parent_sprites_to_sort.push_back(&parent);
		}
		/* El contrato usa el sorter escalar oficial: evita que la arquitectura
		 * del host altere la traza, sin dibujar un framebuffer. */
		ViewportSortParentSprites(&_vd.parent_sprites_to_sort);
		OpenttdrsWorldDrawBeginFinalSort(
			static_cast<uint64_t>(_vd.parent_sprites_to_sort.size()),
			static_cast<uint64_t>(_vd.child_screen_sprites_to_draw.size())
		);
		for (size_t final_ordinal = 0; final_ordinal < _vd.parent_sprites_to_sort.size(); final_ordinal++) {
			const ParentSpriteToDraw &parent = *_vd.parent_sprites_to_sort[final_ordinal];
			const uint64_t parent_id = static_cast<uint64_t>(&parent - _vd.parent_sprites_to_draw.data());
			OpenttdrsWorldDrawRecordFinalParent(
				static_cast<uint64_t>(final_ordinal), parent_id,
				static_cast<uint32_t>(parent.image), static_cast<uint32_t>(parent.pal),
				parent.x, parent.y, parent.left, parent.top,
				parent.xmin, parent.ymin, parent.zmin,
				parent.xmax, parent.ymax, parent.zmax, parent.first_child
			);
			uint64_t child_ordinal = 0;
			for (int child_index = parent.first_child; child_index >= 0; ) {
				const ChildScreenSpriteToDraw &child = _vd.child_screen_sprites_to_draw[child_index];
				const int next = child.next;
				OpenttdrsWorldDrawRecordFinalChild(
					static_cast<uint64_t>(final_ordinal), parent_id, child_ordinal,
					child_index, static_cast<uint32_t>(child.image), static_cast<uint32_t>(child.pal),
					child.x, child.y, child.relative, next
				);
				child_index = next;
				child_ordinal++;
			}
		}
		OpenttdrsWorldDrawFinishFinalSort();
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


def integrate_world_screenshot_zoom(dest: Path) -> None:
    """Expone al oráculo raster un zoom explícito sin cambiar el API normal.

    `SC_DEFAULTZOOM` está fijado a `ZoomLevel::Viewport` en upstream. El
    exportador necesita además `Out2x`/`Out4x` para comparar cada raster con
    la misma escala que openttdrs; integrar una sobrecarga pequeña es menos
    frágil que mutar el viewport principal o capturar la UI con SC_VIEWPORT.
    """
    header = dest / "src" / "screenshot.h"
    header_text = header.read_text(encoding="utf-8")
    if '#include "zoom_type.h"' not in header_text:
        anchor = "#define SCREENSHOT_H\n"
        if anchor not in header_text:
            raise SystemExit("no encuentro guard de screenshot.h")
        header_text = header_text.replace(anchor, anchor + '\n#include "zoom_type.h"\n', 1)

    declaration = (
        "bool MakeScreenshotAtZoom(ZoomLevel zoom, const std::string &name, "
        "uint32_t width = 0, uint32_t height = 0);\n"
    )
    if declaration not in header_text:
        anchor = "bool MakeScreenshot(ScreenshotType t, const std::string &name, uint32_t width = 0, uint32_t height = 0);\n"
        if anchor not in header_text:
            raise SystemExit("no encuentro declaración MakeScreenshot")
        header_text = header_text.replace(anchor, anchor + declaration, 1)
    header.write_text(header_text, encoding="utf-8")

    screenshot = dest / "src" / "screenshot.cpp"
    text = screenshot.read_text(encoding="utf-8")
    if "#include <optional>\n" not in text:
        anchor = '#include "screenshot_type.h"\n'
        if anchor not in text:
            raise SystemExit("no encuentro include screenshot_type.h")
        text = text.replace(anchor, anchor + "\n#include <optional>\n", 1)

    if "std::optional<ZoomLevel> zoom_override" not in text:
        setup_signature = (
            "static Viewport SetupScreenshotViewport(ScreenshotType t, uint32_t width = 0, uint32_t height = 0)\n"
        )
        setup_replacement = (
            "static Viewport SetupScreenshotViewport(ScreenshotType t, uint32_t width = 0, uint32_t height = 0, "
            "std::optional<ZoomLevel> zoom_override = std::nullopt)\n"
        )
        if setup_signature not in text:
            raise SystemExit("no encuentro SetupScreenshotViewport")
        text = text.replace(setup_signature, setup_replacement, 1)

        zoom_assignment = (
            "\t\t\tvp.zoom = (t == SC_ZOOMEDIN) ? _settings_client.gui.zoom_min : ZoomLevel::Viewport;\n"
        )
        zoom_replacement = (
            "\t\t\tvp.zoom = zoom_override.value_or(\n"
            "\t\t\t\t(t == SC_ZOOMEDIN) ? _settings_client.gui.zoom_min : ZoomLevel::Viewport\n"
            "\t\t\t);\n"
        )
        if zoom_assignment not in text:
            raise SystemExit("no encuentro zoom default de SetupScreenshotViewport")
        text = text.replace(zoom_assignment, zoom_replacement, 1)

        large_signature = (
            "static bool MakeLargeWorldScreenshot(ScreenshotType t, uint32_t width = 0, uint32_t height = 0)\n"
        )
        large_replacement = (
            "static bool MakeLargeWorldScreenshot(ScreenshotType t, uint32_t width = 0, uint32_t height = 0, "
            "std::optional<ZoomLevel> zoom_override = std::nullopt)\n"
        )
        if large_signature not in text:
            raise SystemExit("no encuentro MakeLargeWorldScreenshot")
        text = text.replace(large_signature, large_replacement, 1)

        viewport_call = "\tViewport vp = SetupScreenshotViewport(t, width, height);\n"
        if viewport_call not in text:
            raise SystemExit("no encuentro llamada SetupScreenshotViewport")
        text = text.replace(
            viewport_call,
            "\tViewport vp = SetupScreenshotViewport(t, width, height, zoom_override);\n",
            1,
        )

        real_signature = (
            "static bool RealMakeScreenshot(ScreenshotType t, const std::string &name, uint32_t width, uint32_t height)\n"
        )
        real_replacement = (
            "static bool RealMakeScreenshot(ScreenshotType t, const std::string &name, uint32_t width, uint32_t height, "
            "std::optional<ZoomLevel> zoom_override = std::nullopt)\n"
        )
        if real_signature not in text:
            raise SystemExit("no encuentro RealMakeScreenshot")
        text = text.replace(real_signature, real_replacement, 1)

        large_call = "\t\t\tret = MakeLargeWorldScreenshot(t, width, height);\n"
        if large_call not in text:
            raise SystemExit("no encuentro llamada MakeLargeWorldScreenshot")
        text = text.replace(
            large_call,
            "\t\t\tret = MakeLargeWorldScreenshot(t, width, height, zoom_override);\n",
            1,
        )

        # LargeWorldCallback must use the same zoom as the requested viewport.
        # Normal screenshots remain byte-identical because both values are Normal.
        dpi_zoom = "\t\t.zoom = ZoomLevel::WorldScreenshot\n"
        if dpi_zoom not in text:
            raise SystemExit("no encuentro zoom de LargeWorldCallback")
        text = text.replace(dpi_zoom, "\t\t.zoom = vp.zoom\n", 1)

    zoom_api = (
        "\nbool MakeScreenshotAtZoom(ZoomLevel zoom, const std::string &name, uint32_t width, uint32_t height)\n"
        "{\n"
        "\tif (zoom < ZoomLevel::Min || zoom > ZoomLevel::Max) return false;\n"
        "\n"
        "\tVideoDriver::GetInstance()->QueueOnMainThread([=] {\n"
        "\t\tRealMakeScreenshot(SC_DEFAULTZOOM, name, width, height, zoom);\n"
        "\t});\n"
        "\n"
        "\treturn true;\n"
        "}\n"
    )
    if "bool MakeScreenshotAtZoom(ZoomLevel zoom" not in text:
        anchor = "\n\nstatic void MinimapScreenCallback(void *buf, uint y, uint pitch, uint n)\n"
        if anchor not in text:
            raise SystemExit("no encuentro ancla posterior a MakeScreenshot")
        text = text.replace(anchor, zoom_api + anchor, 1)

    screenshot.write_text(text, encoding="utf-8")
    print("screenshot: API de zoom explícito para oráculo raster")


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


def integrate_tree_generation_trace(dest: Path) -> None:
    """Conecta snapshots por fase y la traza de PlaceTree al flujo real."""
    genworld = dest / "src" / "genworld.cpp"
    text = genworld.read_text(encoding="utf-8")
    if '#include "snapshot_export.h"' not in text:
        anchor = '#include "landscape.h"\n'
        if anchor not in text:
            raise SystemExit("no encuentro include landscape.h en genworld.cpp")
        text = text.replace(anchor, anchor + '#include "snapshot_export.h"\n', 1)
    tree_hook = (
        "\t\t\t\tGenerateObjects();\n"
        "\t\t\t\tOpenttdrsMaybeCaptureTreeGenerationStage(false);\n"
        "\t\t\t\tGenerateTrees();\n"
        "\t\t\t\tOpenttdrsMaybeCaptureTreeGenerationStage(true);\n"
    )
    if "OpenttdrsMaybeCaptureTreeGenerationStage" not in text:
        marker = "\t\t\t\tGenerateObjects();\n\t\t\t\tGenerateTrees();\n"
        if marker not in text:
            raise SystemExit("no encuentro GenerateObjects/GenerateTrees en genworld.cpp")
        text = text.replace(marker, tree_hook, 1)
        print("genworld: fixture pre/post GenerateTrees")
    else:
        print("genworld: fixture pre/post GenerateTrees ya presente")

    if 'OpenttdrsMaybeCaptureGenerationStage("landscape")' not in text:
        landscape_marker = "\t\tif (!landscape_generated) {\n"
        if landscape_marker not in text:
            raise SystemExit("no encuentro frontera posterior a GenerateLandscape")
        text = text.replace(
            landscape_marker,
            "\t\tif (landscape_generated) OpenttdrsMaybeCaptureGenerationStage(\"landscape\");\n\n"
            + landscape_marker,
            1,
        )
        print("genworld: snapshot posterior a GenerateLandscape")
    else:
        print("genworld: snapshot GenerateLandscape ya presente")

    if 'OpenttdrsMaybeCaptureGenerationStage("clear")' not in text:
        clear_marker = "\t\t\tGenerateClearTile();\n\t\t\tMap::CountLandTiles();\n"
        if clear_marker not in text:
            raise SystemExit("no encuentro frontera GenerateClearTile")
        text = text.replace(
            clear_marker,
            clear_marker + "\t\t\tOpenttdrsMaybeCaptureGenerationStage(\"clear\");\n",
            1,
        )
        print("genworld: snapshot posterior a GenerateClearTile")
    else:
        print("genworld: snapshot GenerateClearTile ya presente")

    if 'OpenttdrsMaybeCaptureGenerationStage("towns")' not in text:
        towns_marker = "\t\t\t\tGenerateIndustries();\n"
        if towns_marker not in text:
            raise SystemExit("no encuentro frontera posterior a GenerateTowns")
        text = text.replace(
            towns_marker,
            "\t\t\t\tOpenttdrsMaybeCaptureGenerationStage(\"towns\");\n" + towns_marker,
            1,
        )
        print("genworld: snapshot posterior a GenerateTowns")
    else:
        print("genworld: snapshot GenerateTowns ya presente")

    if 'OpenttdrsMaybeCaptureGenerationStage("industries")' not in text:
        industries_marker = "\t\t\t\tGenerateIndustries();\n\t\t\t\tGenerateObjects();\n"
        if industries_marker not in text:
            raise SystemExit("no encuentro frontera posterior a GenerateIndustries")
        text = text.replace(
            industries_marker,
            "\t\t\t\tGenerateIndustries();\n"
            "\t\t\t\tOpenttdrsMaybeCaptureGenerationStage(\"industries\");\n"
            "\t\t\t\tGenerateObjects();\n",
            1,
        )
        print("genworld: snapshot posterior a GenerateIndustries")
    else:
        print("genworld: snapshot GenerateIndustries ya presente")

    if 'OpenttdrsMaybeCaptureGenerationStage("objects")' not in text:
        objects_marker = (
            "\t\t\t\tGenerateObjects();\n"
            "\t\t\t\tOpenttdrsMaybeCaptureTreeGenerationStage(false);\n"
        )
        if objects_marker not in text:
            raise SystemExit("no encuentro frontera posterior a GenerateObjects")
        text = text.replace(
            objects_marker,
            "\t\t\t\tGenerateObjects();\n"
            "\t\t\t\tOpenttdrsMaybeCaptureGenerationStage(\"objects\");\n"
            "\t\t\t\tOpenttdrsMaybeCaptureTreeGenerationStage(false);\n",
            1,
        )
        print("genworld: snapshot posterior a GenerateObjects")
    else:
        print("genworld: snapshot GenerateObjects ya presente")

    if 'OpenttdrsMaybeCaptureGenerationStage("trees")' not in text:
        trees_marker = (
            "\t\t\t\tGenerateTrees();\n"
            "\t\t\t\tOpenttdrsMaybeCaptureTreeGenerationStage(true);\n"
        )
        if trees_marker not in text:
            raise SystemExit("no encuentro frontera posterior a GenerateTrees")
        text = text.replace(
            trees_marker,
            "\t\t\t\tGenerateTrees();\n"
            "\t\t\t\tOpenttdrsMaybeCaptureGenerationStage(\"trees\");\n"
            "\t\t\t\tOpenttdrsMaybeCaptureTreeGenerationStage(true);\n",
            1,
        )
        print("genworld: snapshot posterior a GenerateTrees")
    else:
        print("genworld: snapshot GenerateTrees ya presente")
    genworld.write_text(text, encoding="utf-8")

    tree_cmd = dest / "src" / "tree_cmd.cpp"
    text = tree_cmd.read_text(encoding="utf-8")
    if '#include "snapshot_export.h"' not in text:
        anchor = '#include "stdafx.h"\n'
        if anchor not in text:
            raise SystemExit("no encuentro include stdafx.h en tree_cmd.cpp")
        text = text.replace(anchor, anchor + '#include "snapshot_export.h"\n', 1)
    legacy_trace = "\t\tOpenttdrsTraceTreePlacement(static_cast<uint32_t>(TileX(tile)), static_cast<uint32_t>(TileY(tile)), r);\n"
    text = text.replace(legacy_trace, "", 1)
    if 'OpenttdrsTraceTreePlacement("group"' not in text:
        group_marker = (
            "\t\t\tif (!CanPlantTreesOnTile(cur_tile, true)) continue;\n"
            "\t\t\tif (!IsPointInStarShapedPolygon(x, y, grove)) continue;\n"
            "\n"
            "\t\t\tPlaceTree(cur_tile, r);\n"
        )
        group_replacement = (
            "\t\t\tif (!CanPlantTreesOnTile(cur_tile, true)) continue;\n"
            "\t\t\tif (!IsPointInStarShapedPolygon(x, y, grove)) continue;\n"
            "\n"
            "\t\t\tOpenttdrsTraceTreePlacement(\"group\", static_cast<uint32_t>(TileX(cur_tile)), static_cast<uint32_t>(TileY(cur_tile)), r,\n"
            "\t\t\t\tstatic_cast<uint32_t>(TileX(center_tile)), static_cast<uint32_t>(TileY(center_tile)), true);\n"
            "\t\t\tPlaceTree(cur_tile, r);\n"
        )
        if group_marker not in text:
            raise SystemExit("no encuentro colocación de grupo en tree_cmd.cpp")
        text = text.replace(group_marker, group_replacement, 1)

        same_height_marker = "\t\t/* Place one tree and quit */\n\t\tPlaceTree(cur_tile, r);\n"
        same_height_replacement = (
            "\t\t/* Place one tree and quit */\n"
            "\t\tOpenttdrsTraceTreePlacement(\"same_height\", static_cast<uint32_t>(TileX(cur_tile)), static_cast<uint32_t>(TileY(cur_tile)), r,\n"
            "\t\t\tstatic_cast<uint32_t>(TileX(tile)), static_cast<uint32_t>(TileY(tile)), true);\n"
            "\t\tPlaceTree(cur_tile, r);\n"
        )
        if same_height_marker not in text:
            raise SystemExit("no encuentro colocación misma altura en tree_cmd.cpp")
        text = text.replace(same_height_marker, same_height_replacement, 1)

        random_marker = "\t\tif (CanPlantTreesOnTile(tile, true)) {\n\t\t\tPlaceTree(tile, r);\n"
        random_replacement = (
            "\t\tif (CanPlantTreesOnTile(tile, true)) {\n"
            "\t\t\tOpenttdrsTraceTreePlacement(\"random\", static_cast<uint32_t>(TileX(tile)), static_cast<uint32_t>(TileY(tile)), r, 0, 0, false);\n"
            "\t\t\tPlaceTree(tile, r);\n"
        )
        if random_marker not in text:
            raise SystemExit("no encuentro colocación aleatoria en tree_cmd.cpp")
        text = text.replace(random_marker, random_replacement, 1)
        print("tree_cmd: traza de colocaciones GenerateTrees")
    else:
        print("tree_cmd: traza de colocaciones ya presente")
    if 'OpenttdrsTraceTreePlacement("rainforest"' not in text:
        rainforest_marker = (
            "\t\t\tif (GetTropicZone(tile) == TROPICZONE_RAINFOREST && CanPlantTreesOnTile(tile, false)) {\n"
            "\t\t\t\tPlaceTree(tile, r);\n"
            "\t\t\t}\n"
        )
        rainforest_replacement = (
            "\t\t\tif (GetTropicZone(tile) == TROPICZONE_RAINFOREST && CanPlantTreesOnTile(tile, false)) {\n"
            "\t\t\t\tOpenttdrsTraceTreePlacement(\"rainforest\", static_cast<uint32_t>(TileX(tile)), static_cast<uint32_t>(TileY(tile)), r, 0, 0, false);\n"
            "\t\t\t\tPlaceTree(tile, r);\n"
            "\t\t\t}\n"
        )
        if rainforest_marker not in text:
            raise SystemExit("no encuentro colocación rainforest en tree_cmd.cpp")
        text = text.replace(rainforest_marker, rainforest_replacement, 1)
        print("tree_cmd: traza de pase rainforest")
    else:
        print("tree_cmd: traza de pase rainforest ya presente")
    tree_cmd.write_text(text, encoding="utf-8")


def integrate_industry_generation_trace(dest: Path) -> None:
    """Conecta la traza de cada `CreateNewIndustry` al oráculo por fases."""
    industry_cmd = dest / "src" / "industry_cmd.cpp"
    text = industry_cmd.read_text(encoding="utf-8")
    if '#include "snapshot_export.h"' not in text:
        anchor = '#include "industry.h"\n'
        if anchor not in text:
            raise SystemExit("no encuentro include industry.h en industry_cmd.cpp")
        text = text.replace(anchor, anchor + '#include "snapshot_export.h"\n', 1)

    hook = "OpenttdrsTraceIndustryCreationAttempt("
    if hook not in text:
        marker = (
            "\tuint32_t seed = Random();\n"
            "\tuint32_t seed2 = Random();\n"
            "\tIndustry *i = nullptr;\n"
            "\tsize_t layout_index = RandomRange((uint32_t)indspec->layouts.size());\n"
            "\t[[maybe_unused]] CommandCost ret = CreateNewIndustryHelper(tile, type, DoCommandFlag::Execute, indspec, layout_index, seed, GB(seed2, 0, 16), OWNER_NONE, creation_type, &i);\n"
            "\tassert(i != nullptr || ret.Failed());\n"
            "\treturn i;\n"
        )
        replacement = (
            "\tuint32_t seed = Random();\n"
            "\tuint32_t seed2 = Random();\n"
            "\tconst uint16_t initial_random_bits = GB(seed2, 0, 16);\n"
            "\tIndustry *i = nullptr;\n"
            "\tsize_t layout_index = RandomRange((uint32_t)indspec->layouts.size());\n"
            "\t[[maybe_unused]] CommandCost ret = CreateNewIndustryHelper(tile, type, DoCommandFlag::Execute, indspec, layout_index, seed, initial_random_bits, OWNER_NONE, creation_type, &i);\n"
            "\tassert(i != nullptr || ret.Failed());\n"
            "\tOpenttdrsTraceIndustryCreationAttempt(static_cast<uint16_t>(type), static_cast<uint32_t>(TileX(tile)), static_cast<uint32_t>(TileY(tile)), seed, initial_random_bits, static_cast<uint32_t>(layout_index), i != nullptr);\n"
            "\treturn i;\n"
        )
        if marker not in text:
            raise SystemExit("no encuentro CreateNewIndustry para trazar intentos")
        text = text.replace(marker, replacement, 1)
        print("industry_cmd: traza CreateNewIndustry")
    else:
        print("industry_cmd: traza CreateNewIndustry ya presente")
    industry_cmd.write_text(text, encoding="utf-8")


if mode == "world_raw_only":
    # Un árbol no pinneado nuevo sólo recibe world-raw/semantic/draw/raster.
    # Pero un fork que ya descendía del pin puede contener los hooks de
    # snapshot en genworld/tree/openttd; retirarle snapshot_export.cpp dejaría
    # referencias sin resolver. En ese caso conservamos la instrumentación ya
    # presente y añadimos el raster sin degradar el build existente.
    snapshot_source = dest / "src" / "snapshot_export.cpp"
    snapshot_dependent_markers = (
        "OpenttdrsMaybeCaptureGenerationStage",
        "OpenttdrsMaybeCaptureTreeGenerationStage",
        "OpenttdrsTraceTreePlacement",
        "OpenttdrsTraceIndustryCreationAttempt",
        "OpenttdrsMaybeExportPbsTraceTick",
        "OpenttdrsMaybeExportAirportFtaTraceTick",
    )
    snapshot_dependent_files = (
        dest / "src" / "genworld.cpp",
        dest / "src" / "tree_cmd.cpp",
        dest / "src" / "industry_cmd.cpp",
        dest / "src" / "openttd.cpp",
    )
    preserve_snapshot_export = snapshot_source.exists() and any(
        marker in path.read_text(encoding="utf-8")
        for path in snapshot_dependent_files
        for marker in snapshot_dependent_markers
    )
    if preserve_snapshot_export:
        # El fork puede conservar una versión anterior de nuestro exportador.
        # Preservar sus hooks sin sincronizar las fuentes hacía que un cambio
        # versionado del contrato no llegara al binario que se usa como
        # oráculo. El parche es la fuente canónica de ese exportador; sólo se
        # copia cuando el árbol ya demostró ser descendiente instrumentado.
        for name in ("snapshot_export.cpp", "snapshot_export.h"):
            source = patch_dir / "src" / name
            target = dest / "src" / name
            if target.read_bytes() != source.read_bytes():
                target.write_bytes(source.read_bytes())
                print(f"snapshot_export: {name} sincronizado desde el parche")
        integrate_industry_generation_trace(dest)

    cmake = dest / "src" / "CMakeLists.txt"
    text = cmake.read_text(encoding="utf-8")
    if preserve_snapshot_export:
        text = add_cmake_source(cmake, text, "snapshot_export.cpp")
        print("CMakeLists: se conserva snapshot_export.cpp ya requerido")
    else:
        text = text.replace("    snapshot_export.cpp\n", "", 1)
    if preserve_snapshot_export:
        # La variante completa de snapshot 15.3 ya define world-raw. Añadir
        # nuestro exportador mínimo además de ella duplicaría el símbolo al
        # enlazar un fork que conserva esa instrumentación.
        text = text.replace("    world_raw_export.cpp\n", "", 1)
        print("CMakeLists: se omite world_raw_export.cpp duplicado de snapshot")
    elif "world_raw_export.cpp" not in text:
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
    snapshot_include = '#include "../snapshot_export.h"\n'
    if preserve_snapshot_export and snapshot_include not in at:
        nl = at.find("\n#include ")
        if nl < 0:
            raise SystemExit("no encuentro includes en afterload.cpp para snapshot existente")
        at = at[: nl + 1] + snapshot_include + at[nl + 1 :]
        print("afterload: se conserva include snapshot existente")
    elif not preserve_snapshot_export:
        at = at.replace('#include "../snapshot_export.h"\n', "", 1)
    if preserve_snapshot_export:
        # snapshot_export.h ya declara world-raw en la variante completa.
        # Conservar además el header mínimo produce una redeclaración inútil
        # al compilar un fork derivado del pin.
        at = at.replace('#include "../world_raw_export.h"\n', "", 1)
    elif '#include "../world_raw_export.h"' not in at:
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
    snapshot_hooks = (snapshot_hook + pbs_hook + fta_hook) if preserve_snapshot_export else ""
    at = at.replace(
        anchor,
        snapshot_hooks + raw_hook + semantic_hook + draw_hook + screenshot_hook
        + "\treturn true;\n}\n\n/**\n * Reload all NewGRF",
        1,
    )
    after.write_text(at, encoding="utf-8")
    integrate_world_draw_viewport(dest)
    integrate_world_draw_foundation(dest)
    integrate_world_screenshot_zoom(dest)
    integrate_headless_raster_blitter(dest)
    print("afterload: hooks world-raw/world-semantic/world-draw/world-screenshot AfterLoadGame")

    openttd = dest / "src" / "openttd.cpp"
    ot = openttd.read_text(encoding="utf-8")
    if preserve_snapshot_export:
        snapshot_include = '#include "snapshot_export.h"\n'
        if snapshot_include not in ot:
            anchor = '#include "stdafx.h"\n'
            if anchor not in ot:
                raise SystemExit("no encuentro include stdafx.h en openttd.cpp para snapshot existente")
            ot = ot.replace(anchor, snapshot_include + anchor, 1)
        tick_anchor = "\t\tcur_company.Restore();\n"
        pbs_tick_hook = "\tOpenttdrsMaybeExportPbsTraceTick();\n"
        fta_tick_hook = "\tOpenttdrsMaybeExportAirportFtaTraceTick();\n"
        if pbs_tick_hook not in ot:
            if tick_anchor not in ot:
                raise SystemExit("no encuentro ancla post-tick para snapshot existente")
            ot = ot.replace(tick_anchor, tick_anchor + pbs_tick_hook + fta_tick_hook, 1)
        elif fta_tick_hook not in ot:
            ot = ot.replace(pbs_tick_hook, pbs_tick_hook + fta_tick_hook, 1)
        openttd.write_text(ot, encoding="utf-8")
        print("openttd: se conservan hooks snapshot/PBS/FTA ya requeridos")
        print("Integración world-raw/world-semantic/world-draw/world-screenshot lista sobre fork con snapshot existente")
    else:
        # El modo no pinneado no compila snapshot_export.cpp; retirar los
        # hooks post-tick junto al include evita referencias a símbolos fuera
        # de alcance si el árbol los hubiera añadido manualmente.
        pbs_tick_hook = "\tOpenttdrsMaybeExportPbsTraceTick();\n"
        fta_tick_hook = "\tOpenttdrsMaybeExportAirportFtaTraceTick();\n"
        removed_ticks = 0
        for tick_hook in (pbs_tick_hook, fta_tick_hook):
            if tick_hook in ot:
                ot = ot.replace(tick_hook, "", 1)
                removed_ticks += 1
        cleaned = ot.replace('#include "snapshot_export.h"\n', "", 1)
        if cleaned != ot or removed_ticks:
            openttd.write_text(cleaned, encoding="utf-8")
        if cleaned != ot:
            print("openttd: se retiró include snapshot incompatible")
        if removed_ticks:
            print("openttd: se retiraron hooks PBS/FTA incompatibles")
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

integrate_tree_generation_trace(dest)
integrate_industry_generation_trace(dest)

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
integrate_world_screenshot_zoom(dest)
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
