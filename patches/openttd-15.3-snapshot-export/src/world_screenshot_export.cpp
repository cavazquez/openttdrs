/*
 * Reproducible raster reference for openttdrs world-render parity.
 *
 * The regular screenshot implementation owns image encoding and the target
 * screenshot directory. This helper only centers the main viewport, queues a
 * explicitly zoomed viewport render, copies the resulting PNG to an explicit
 * path, and exits once the queued render completed.
 */

#include "world_screenshot_export.h"

#include "map_func.h"
#include "openttd.h"
#include "screenshot.h"
#include "transparency.h"
#include "viewport_func.h"
#include "video/video_driver.hpp"
#include "window_func.h"

#include "3rdparty/nlohmann/json.hpp"

#include <chrono>
#include <charconv>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <optional>
#include <string>
#include <string_view>

using nlohmann::json;

namespace {

struct ScreenshotSortTraceState {
	bool active = false;
	bool failed = false;
	bool in_segment = false;
	uint32_t expected_width = 0;
	uint32_t expected_height = 0;
	int expected_zoom = 0;
	uint64_t segments = 0;
	uint64_t parents = 0;
	std::ofstream out;
};

ScreenshotSortTraceState _openttdrs_world_screenshot_sort_trace;

void EmitScreenshotSortTrace(json row)
{
	auto &state = _openttdrs_world_screenshot_sort_trace;
	if (!state.active || state.failed) return;
	state.out << row.dump() << '\n';
	if (!state.out) state.failed = true;
}

bool ParseUint(std::string_view text, uint32_t &value)
{
	const auto [end, error] = std::from_chars(text.data(), text.data() + text.size(), value);
	return error == std::errc{} && end == text.data() + text.size();
}

bool ParseCenter(const char *raw, uint32_t &x, uint32_t &y)
{
	if (raw == nullptr || raw[0] == '\0') return false;
	const std::string_view text(raw);
	const size_t separator = text.find(',');
	if (separator == std::string_view::npos) return false;
	return ParseUint(text.substr(0, separator), x) && ParseUint(text.substr(separator + 1), y);
}

bool ParseResolution(const char *raw, uint32_t &width, uint32_t &height)
{
	if (raw == nullptr || raw[0] == '\0') return true;
	const std::string_view text(raw);
	const size_t separator = text.find('x');
	if (separator == std::string_view::npos ||
			!ParseUint(text.substr(0, separator), width) ||
			!ParseUint(text.substr(separator + 1), height)) {
		return false;
	}
	return width > 0 && height > 0;
}

/**
 * La escala es la misma convención que el candidato: factor ortográfico de
 * openttdrs, no el texto inverso que muestra su HUD. Mantener esta tabla aquí
 * evita que una referencia `Out2x` se compare accidentalmente con candidata
 * normal.
 */
bool ParseScreenshotScale(const char *raw, ZoomLevel &zoom)
{
	if (raw == nullptr || raw[0] == '\0') {
		zoom = ZoomLevel::Normal;
		return true;
	}

	const std::string_view value(raw);
	if (value == "0.25") {
		zoom = ZoomLevel::In4x;
	} else if (value == "0.5") {
		zoom = ZoomLevel::In2x;
	} else if (value == "1") {
		zoom = ZoomLevel::Normal;
	} else if (value == "2") {
		zoom = ZoomLevel::Out2x;
	} else if (value == "4") {
		zoom = ZoomLevel::Out4x;
	} else if (value == "8") {
		zoom = ZoomLevel::Out8x;
	} else {
		return false;
	}
	return true;
}

bool EnvEnabled(const char *name)
{
	const char *raw = std::getenv(name);
	if (raw == nullptr || raw[0] == '\0') return false;
	const std::string_view value(raw);
	return value != "0" && value != "false" && value != "no" && value != "off";
}

int WorldScreenshotMinCall()
{
	const char *raw = std::getenv("OPENTTDRS_WORLD_SCREENSHOT_MIN_CALL");
	if (raw == nullptr || raw[0] == '\0') return 2;
	const int requested = std::atoi(raw);
	return requested > 0 ? requested : 2;
}

/**
 * La captura reusa la esquina virtual del viewport principal, pero le puede
 * pedir al raster un tamaño y zoom distintos a los de la ventana headless. Si
 * no corregimos esa esquina, `ScrollMainWindowToTile` centra la tesela en el
 * viewport original y la captura recortada queda desplazada. Mantener el
 * centro virtual evita que el oráculo compare regiones distintas al cambiar
 * la resolución o la escala.
 */
void CenterScreenshotViewportOnMainWindow(
	Window &window, uint32_t width, uint32_t height, ZoomLevel zoom
)
{
	ViewportData &viewport = *window.viewport;
	const uint32_t zoom_factor = 1U << to_underlying(zoom);
	const int requested_virtual_width = static_cast<int>(width * zoom_factor);
	const int requested_virtual_height = static_cast<int>(height * zoom_factor);
	const int delta_x = (viewport.virtual_width - requested_virtual_width) / 2;
	const int delta_y = (viewport.virtual_height - requested_virtual_height) / 2;
	/* `UpdateViewportPosition` deriva virtual_left/top de scrollpos. Ajustar
	 * solamente los campos virtuales duraría hasta el siguiente tick; mover
	 * ambas posiciones de scroll conserva el recorte hasta que MakeScreenshot
	 * consume el viewport en la tarea encolada. */
	viewport.scrollpos_x += delta_x;
	viewport.dest_scrollpos_x += delta_x;
	viewport.scrollpos_y += delta_y;
	viewport.dest_scrollpos_y += delta_y;
}

void LogScreenshotViewport(const Window &window, uint32_t width, uint32_t height, ZoomLevel zoom)
{
	if (!EnvEnabled("OPENTTDRS_WORLD_SCREENSHOT_DEBUG")) return;
	const ViewportData &viewport = *window.viewport;
	std::fprintf(stderr,
		"openttdrs world-screenshot: scroll=(%d,%d) dest=(%d,%d) virtual=(%d,%d %dx%d) capture=%ux%u zoom=%d\n",
		viewport.scrollpos_x, viewport.scrollpos_y,
		viewport.dest_scrollpos_x, viewport.dest_scrollpos_y,
		viewport.virtual_left, viewport.virtual_top,
		viewport.virtual_width, viewport.virtual_height, width, height,
		to_underlying(zoom));
}

/**
 * Normaliza las capas que son inherentemente temporales o configurables para
 * que una captura de paridad mida terreno e infraestructura, no nombres de
 * pueblos/estaciones ni la carrera entre dos loops de simulación.
 */
void PrepareCleanWorldScreenshot()
{
	_pause_mode.Set(PauseMode::Normal);
	ClrBit(_display_opt, DO_SHOW_TOWN_NAMES);
	ClrBit(_display_opt, DO_SHOW_STATION_NAMES);
	ClrBit(_display_opt, DO_SHOW_WAYPOINT_NAMES);
	ClrBit(_display_opt, DO_SHOW_SIGNS);
	ClrBit(_display_opt, DO_SHOW_COMPETITOR_SIGNS);
	ClrBit(_display_opt, DO_FULL_ANIMATION);
}

} // namespace

bool OpenttdrsMaybeCaptureWorldScreenshot()
{
	const char *output = std::getenv("OPENTTDRS_WORLD_SCREENSHOT_OUT");
	if (output == nullptr || output[0] == '\0') return true;

	/* Dedicated + -g primero carga una partida temporal. Igual que los
	 * exportadores raw/semantic/draw, ignorar ese primer AfterLoadGame evita
	 * que el PNG pertenezca al mapa de arranque en lugar del .sav solicitado. */
	static int call_count = 0;
	call_count++;
	if (call_count < WorldScreenshotMinCall()) return true;

	if (EnvEnabled("OPENTTDRS_WORLD_SCREENSHOT_CLEAN")) {
		PrepareCleanWorldScreenshot();
	}

	uint32_t width = 1280;
	uint32_t height = 720;
	if (!ParseResolution(std::getenv("OPENTTDRS_WORLD_SCREENSHOT_RES"), width, height)) {
		std::fprintf(stderr, "openttdrs world-screenshot: resolución inválida (usar anchoxalto)\n");
		return false;
	}

	ZoomLevel zoom = ZoomLevel::Normal;
	if (!ParseScreenshotScale(std::getenv("OPENTTDRS_WORLD_SCREENSHOT_SCALE"), zoom)) {
		std::fprintf(stderr,
			"openttdrs world-screenshot: escala inválida (usar 0.25, 0.5, 1, 2, 4 u 8)\n");
		return false;
	}

	std::optional<TileIndex> center;
	if (const char *raw_center = std::getenv("OPENTTDRS_WORLD_SCREENSHOT_CENTER"); raw_center != nullptr) {
		uint32_t x = 0;
		uint32_t y = 0;
		if (!ParseCenter(raw_center, x, y) || x >= Map::SizeX() || y >= Map::SizeY()) {
			std::fprintf(stderr, "openttdrs world-screenshot: centro inválido %s (usar x,y dentro del mapa)\n", raw_center);
			return false;
		}
		center = TileXY(x, y);
	}

	const std::string target(output);
	/* En el driver dedicado, AfterLoadGame encola esto durante Tick N y el
	 * primer callback corre justo antes de Tick N+1, cuando OpenTTD todavía
	 * puede restaurar la cámara guardada. Diferimos un callback adicional:
	 * entonces el centrado se hace antes de Tick N+2, ya estable. */
	VideoDriver::GetInstance()->QueueOnMainThread([center, width, height, zoom, target] {
		VideoDriver::GetInstance()->QueueOnMainThread([center, width, height, zoom, target] {
		if (center.has_value()) {
			const bool moved = ScrollMainWindowToTile(*center, true);
			if (EnvEnabled("OPENTTDRS_WORLD_SCREENSHOT_DEBUG")) {
				std::fprintf(stderr, "openttdrs world-screenshot: focus=(%u,%u) moved=%d\n",
					TileX(*center), TileY(*center), moved);
			}
			/* El scroll instantáneo actualiza scrollpos, pero la captura de
			 * viewport consume virtual_left/virtual_top. En una ejecución
			 * headless no esperamos el siguiente DrawOverlappedWindow; forzamos
			 * la misma actualización que haría ese frame antes de capturar. */
			if (Window *main_window = GetMainWindow(); main_window != nullptr) {
				UpdateViewportPosition(main_window, 0);
				CenterScreenshotViewportOnMainWindow(*main_window, width, height, zoom);
				UpdateViewportPosition(main_window, 0);
				LogScreenshotViewport(*main_window, width, height, zoom);
			}
		}

		/* `-x` deliberately starts from a blank config while exporting a reference.
		 * The normal setting therefore has an empty screenshot format, which makes
		 * the screenshot provider lookup fail. PNG is built into our reference
		 * configuration and gives the comparison script a deterministic artifact. */
		_screenshot_format_name = "png";
		/* MakeScreenshot only reports that its work was queued. Give each
		 * request a fresh internal name so that a failed queued raster cannot
		 * make us copy an older successful PNG from a previous invocation. */
		const std::string screenshot_name = "openttdrs-world-reference-" +
			std::to_string(std::chrono::steady_clock::now().time_since_epoch().count());
		if (!OpenttdrsWorldScreenshotStartSortTrace(
			width, height, static_cast<int>(to_underlying(zoom))
		)) {
			std::fprintf(stderr, "openttdrs world-screenshot: no se pudo abrir la traza del sorter\n");
			_exit_game = true;
			return;
		}
		if (!MakeScreenshotAtZoom(zoom, screenshot_name, width, height)) {
			std::fprintf(stderr, "openttdrs world-screenshot: no se pudo encolar la captura\n");
			_exit_game = true;
			return;
		}

		/* MakeScreenshot encola primero el raster. Esta segunda tarea queda
		 * detrás de él, por lo que `_full_screenshot_path` ya corresponde a
		 * esta captura. Si el raster falló no existe un archivo con el nombre
		 * nuevo: abortar es preferible a copiar una referencia obsoleta. */
		VideoDriver::GetInstance()->QueueOnMainThread([target] {
			if (!OpenttdrsWorldScreenshotFinishSortTrace()) {
				std::fprintf(stderr, "openttdrs world-screenshot: la traza del sorter quedó incompleta\n");
				_exit_game = true;
				return;
			}
			std::error_code error;
			const std::filesystem::path source(_full_screenshot_path);
			if (!std::filesystem::is_regular_file(source, error) || error ||
					std::filesystem::file_size(source, error) == 0 || error) {
				std::fprintf(stderr, "openttdrs world-screenshot: el raster no produjo PNG nuevo\n");
				_exit_game = true;
				return;
			}
			const std::filesystem::path destination(target);
			if (!destination.parent_path().empty()) {
				std::filesystem::create_directories(destination.parent_path(), error);
			}
			if (!error) {
				std::filesystem::copy_file(
					source,
					destination,
					std::filesystem::copy_options::overwrite_existing,
					error
				);
			}
			if (error) {
				std::fprintf(stderr, "openttdrs world-screenshot: no se pudo copiar a %s: %s\n", target.c_str(), error.message().c_str());
			} else {
				std::fprintf(stderr, "openttdrs world-screenshot: escrito %s\n", target.c_str());
			}
			_exit_game = true;
		});
		});
	});
	return true;
}

bool OpenttdrsWorldScreenshotHideVehicles()
{
	return EnvEnabled("OPENTTDRS_WORLD_SCREENSHOT_CLEAN");
}

bool OpenttdrsWorldScreenshotStartSortTrace(uint32_t width, uint32_t height, int zoom)
{
	const char *output = std::getenv("OPENTTDRS_WORLD_SCREENSHOT_SORT_OUT");
	if (output == nullptr || output[0] == '\0') return true;

	auto &state = _openttdrs_world_screenshot_sort_trace;
	if (state.active) return false;
	state = {};

	std::error_code error;
	const std::filesystem::path destination(output);
	if (!destination.parent_path().empty()) {
		std::filesystem::create_directories(destination.parent_path(), error);
	}
	if (error) return false;
	state.out.open(destination, std::ios::out | std::ios::trunc);
	if (!state.out) return false;

	state.active = true;
	state.expected_width = width;
	state.expected_height = height;
	state.expected_zoom = zoom;
	EmitScreenshotSortTrace({
		{"kind", "metadata"},
		{"schema_version", 1},
		{"contract", "world-screenshot-sort"},
		{"producer", "openttd"},
		{"stage", "post_viewport_sprite_sorter"},
		{"width", width},
		{"height", height},
		{"zoom", zoom},
	});
	state.out.flush();
	if (!state.out) state.failed = true;
	return !state.failed;
}

bool OpenttdrsWorldScreenshotFinishSortTrace()
{
	auto &state = _openttdrs_world_screenshot_sort_trace;
	if (!state.active) return true;
	if (state.in_segment || state.segments == 0) state.failed = true;
	if (!state.failed) {
		EmitScreenshotSortTrace({
			{"kind", "complete"},
			{"segments", state.segments},
			{"parents", state.parents},
		});
	}
	state.out.flush();
	const bool ok = !state.failed && static_cast<bool>(state.out);
	state.out.close();
	state.active = false;
	return ok;
}

bool OpenttdrsWorldScreenshotSortTraceMatches(
	int viewport_left, int viewport_top, int viewport_width, int viewport_height, int zoom
)
{
	const auto &state = _openttdrs_world_screenshot_sort_trace;
	return state.active && !state.failed &&
		viewport_left == 0 && viewport_top == 0 &&
		viewport_width == static_cast<int>(state.expected_width) &&
		viewport_height == static_cast<int>(state.expected_height) &&
		zoom == state.expected_zoom;
}

void OpenttdrsWorldScreenshotBeginSortSegment(
	int virtual_left, int virtual_top, int virtual_width, int virtual_height,
	uint64_t parent_count, uint64_t child_count
)
{
	auto &state = _openttdrs_world_screenshot_sort_trace;
	if (!state.active || state.failed) return;
	if (state.in_segment) {
		state.failed = true;
		return;
	}
	state.in_segment = true;
	state.parents += parent_count;
	EmitScreenshotSortTrace({
		{"kind", "segment"},
		{"ordinal", state.segments},
		{"virtual", {
			{"left", virtual_left}, {"top", virtual_top},
			{"width", virtual_width}, {"height", virtual_height},
		}},
		{"parents", parent_count},
		{"children", child_count},
	});
}

void OpenttdrsWorldScreenshotRecordSortParent(
	uint64_t final_ordinal, uint64_t parent_id,
	uint32_t image, uint32_t palette,
	int screen_x, int screen_y, int left, int top,
	int xmin, int ymin, int zmin, int xmax, int ymax, int zmax, int first_child
)
{
	const auto &state = _openttdrs_world_screenshot_sort_trace;
	if (!state.active || state.failed || !state.in_segment) return;
	EmitScreenshotSortTrace({
		{"kind", "parent"},
		{"segment", state.segments},
		{"final_ordinal", final_ordinal},
		{"parent_id", parent_id},
		{"image", image},
		{"palette", palette},
		{"screen", {{"x", screen_x}, {"y", screen_y}, {"left", left}, {"top", top}}},
		{"world_bounds", {
			{"xmin", xmin}, {"ymin", ymin}, {"zmin", zmin},
			{"xmax", xmax}, {"ymax", ymax}, {"zmax", zmax},
		}},
		{"first_child", first_child},
	});
}

void OpenttdrsWorldScreenshotFinishSortSegment()
{
	auto &state = _openttdrs_world_screenshot_sort_trace;
	if (!state.active || state.failed) return;
	if (!state.in_segment) {
		state.failed = true;
		return;
	}
	state.in_segment = false;
	state.segments++;
}
