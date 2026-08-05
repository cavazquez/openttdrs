/*
 * UI capture driver for openttdrs visual parity (#297).
 *
 * It is deliberately small and only drives the first visual-regression
 * family.  The save selected by the caller supplies the town, industry,
 * depot, and vehicle; this driver never creates game state of its own.
 */

#include "ui_capture.h"

#include "depot_base.h"
#include "depot_func.h"
#include "depot_map.h"
#include "gui.h"
#include "industry.h"
#include "object.h"
#include "openttd.h"
#include "rail_gui.h"
#include "road_gui.h"
#include "screenshot.h"
#include "terraform_gui.h"
#include "timetable.h"
#include "town.h"
#include "vehicle_base.h"
#include "vehicle_gui.h"

#include "widgets/airport_widget.h"
#include "widgets/dock_widget.h"
#include "widgets/rail_widget.h"
#include "widgets/road_widget.h"
#include "widgets/terraform_widget.h"

#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <string_view>

/* `industry_gui.cpp` does not expose this declaration in a public header. */
void ShowIndustryViewWindow(IndustryID industry);

namespace {

enum class CapturePhase : uint8_t {
	Idle,
	WindowOpened,
	Settling,
	Queued,
	DrainOne,
	DrainTwo,
	Done,
};

CapturePhase phase = CapturePhase::Idle;
uint8_t settle_frames = 0;

bool IsSafeScreenshotName(std::string_view value)
{
	if (value.empty() || value.size() > 120) return false;
	for (const unsigned char c : value) {
		if (!(std::isalnum(c) || c == '_' || c == '-')) return false;
	}
	return true;
}

const Vehicle *FirstPrimaryVehicle()
{
	for (const Vehicle *vehicle : Vehicle::Iterate()) {
		if (vehicle->IsPrimaryVehicle() && vehicle->tile != INVALID_TILE) return vehicle;
	}
	return nullptr;
}

bool ClickPicker(Window *toolbar, WidgetID widget)
{
	if (toolbar == nullptr) return false;
	/* The toolbar handlers use the widget ID, not the pointer position, to
	 * choose the construction tool.  This keeps captures independent from
	 * desktop placement and from theme-specific widget bounds. */
	toolbar->OnClick({0, 0}, widget, 1);
	return true;
}

bool OpenConstructionPicker(std::string_view id)
{
	if (id == "RailStationPicker") {
		return ClickPicker(ShowBuildRailToolbar(RAILTYPE_RAIL), WID_RAT_BUILD_STATION);
	}
	if (id == "AirportPicker") {
		return ClickPicker(ShowBuildAirToolbar(), WID_AT_AIRPORT);
	}
	if (id == "RoadStopPicker") {
		return ClickPicker(ShowBuildRoadToolbar(ROADTYPE_ROAD), WID_ROT_BUS_STATION);
	}
	if (id == "ObjectPicker") {
		/* OpenGFX does not ship object specs. Route through the real toolbar so
		 * the reference records the deterministic disabled/empty selector state
		 * instead of pretending that a NewGRF object exists in the fixture. */
		return ClickPicker(ShowTerraformToolbar(), WID_TT_PLACE_OBJECT);
	}
	if (id == "BridgePicker") {
		/* The bridge chooser is normally reached after selecting a two-tile
		 * stretch.  These adjacent map tiles only provide deterministic preview
		 * dimensions; no command is posted while the capture is paused. */
		ShowBuildBridgeWindow(TileXY(1, 1), TileXY(2, 1), TRANSPORT_RAIL, RAILTYPE_RAIL);
		return true;
	}
	if (id == "DockPicker") {
		return ClickPicker(ShowBuildDocksToolbar(), WID_DT_STATION);
	}
	if (id == "BuoyPicker") {
		return ClickPicker(ShowBuildDocksToolbar(), WID_DT_BUOY);
	}
	if (id == "RailWaypointPicker") {
		return ClickPicker(ShowBuildRailToolbar(RAILTYPE_RAIL), WID_RAT_BUILD_WAYPOINT);
	}
	if (id == "RoadWaypointPicker") {
		return ClickPicker(ShowBuildRoadToolbar(ROADTYPE_ROAD), WID_ROT_BUILD_WAYPOINT);
	}
	if (id == "TreePicker") {
		ShowBuildTreesToolbar();
		return true;
	}
	if (id == "TerraformPicker") return ShowTerraformToolbar() != nullptr;
	if (id == "SignPicker") {
		return ClickPicker(ShowTerraformToolbar(), WID_TT_PLACE_SIGN);
	}
	if (id == "DepotBuildPicker") {
		return ClickPicker(ShowBuildRailToolbar(RAILTYPE_RAIL), WID_RAT_BUILD_DEPOT);
	}
	if (id == "SignalPicker") {
		return ClickPicker(ShowBuildRailToolbar(RAILTYPE_RAIL), WID_RAT_BUILD_SIGNALS);
	}
	return false;
}

bool OpenCaptureWindow(std::string_view id)
{
	if (OpenConstructionPicker(id)) return true;

	if (id == "Vehicle" || id == "Orders" || id == "Timetable") {
		const Vehicle *vehicle = FirstPrimaryVehicle();
		if (vehicle == nullptr) return false;
		if (id == "Vehicle") ShowVehicleViewWindow(vehicle);
		if (id == "Orders") ShowOrdersWindow(vehicle);
		if (id == "Timetable") ShowTimetableWindow(vehicle);
		return true;
	}

	if (id == "Depot") {
		for (const Depot *depot : Depot::Iterate()) {
			if (depot->xy == INVALID_TILE || !IsDepotTile(depot->xy)) continue;
			ShowDepotWindow(depot->xy, GetDepotVehicleType(depot->xy));
			return true;
		}
		return false;
	}

	if (id == "Town") {
		for (const Town *town : Town::Iterate()) {
			ShowTownViewWindow(town->index);
			return true;
		}
		return false;
	}

	if (id == "Industry") {
		for (const Industry *industry : Industry::Iterate()) {
			ShowIndustryViewWindow(industry->index);
			return true;
		}
		return false;
	}

	return false;
}

} // namespace

void OpenttdrsMaybeCaptureUi()
{
	const char *id = std::getenv("OPENTTDRS_UI_CAPTURE_ID");
	const char *name = std::getenv("OPENTTDRS_UI_CAPTURE_NAME");
	if (id == nullptr || id[0] == '\0' || name == nullptr || !IsSafeScreenshotName(name)) return;

	switch (phase) {
		case CapturePhase::Idle:
			/* The integration hook runs before the pause guard in StateGameLoop. */
			/* Keep simulation and animated widgets fixed while the frames settle. */
			_pause_mode.Set(PauseMode::Normal);
			if (!OpenCaptureWindow(id)) {
				std::fprintf(stderr, "openttdrs UI capture: fixture lacks target %s\n", id);
				_exit_game = true;
				phase = CapturePhase::Done;
				return;
			}
			phase = CapturePhase::WindowOpened;
			return;

		case CapturePhase::WindowOpened:
		case CapturePhase::Settling:
			/* The UI is rendered outside StateGameLoop; let it present enough
			 * paused frames to settle widgets and text before queuing the PNG. */
			if (++settle_frames < 20) {
				phase = CapturePhase::Settling;
				return;
			}
			if (!MakeScreenshot(SC_VIEWPORT, name)) {
				std::fprintf(stderr, "openttdrs UI capture: screenshot queue rejected\n");
				_exit_game = true;
				phase = CapturePhase::Done;
				return;
			}
			phase = CapturePhase::Queued;
			return;

		case CapturePhase::Queued:
			phase = CapturePhase::DrainOne;
			return;

		case CapturePhase::DrainOne:
			phase = CapturePhase::DrainTwo;
			return;

		case CapturePhase::DrainTwo:
			_exit_game = true;
			phase = CapturePhase::Done;
			return;

		case CapturePhase::Done:
			return;
	}
}
