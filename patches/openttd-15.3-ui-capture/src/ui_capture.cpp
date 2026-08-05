/*
 * UI capture driver for openttdrs visual parity (#297, #299, #300, #301).
 *
 * It is deliberately small and only drives the first visual-regression
 * family.  The save selected by the caller supplies the town, industry,
 * depot, and vehicle; this driver never creates game state of its own.
 */

#include "ui_capture.h"

#include "depot_base.h"
#include "depot_func.h"
#include "depot_map.h"
#include "cheat_func.h"
#include "error.h"
#include "gui.h"
#include "help_gui.h"
#include "company_base.h"
#include "company_gui.h"
#include "graph_gui.h"
#include "industry.h"
#include "league_gui.h"
#include "news_gui.h"
#include "newgrf_config.h"
#include "object.h"
#include "openttd.h"
#include "querystring_gui.h"
#include "rail_gui.h"
#include "road_gui.h"
#include "screenshot.h"
#include "terraform_gui.h"
#include "textbuf_gui.h"
#include "strings_func.h"
#include "timetable.h"
#include "town.h"
#include "vehicle_base.h"
#include "vehicle_gui.h"
#include "window_func.h"

#include "ai/ai_gui.hpp"

#include "widgets/airport_widget.h"
#include "widgets/dock_widget.h"
#include "widgets/rail_widget.h"
#include "widgets/road_widget.h"
#include "widgets/settings_widget.h"
#include "widgets/terraform_widget.h"

#include "table/strings.h"

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

Company *FirstCompany()
{
	for (Company *company : Company::Iterate()) {
		return company;
	}
	return nullptr;
}

void PrepareCompanyIdentityForCapture(Company *company)
{
	/* The compact fixture stores an obsolete generated company-name string.
	 * Finance windows tolerate it, but CompanyWindow may dereference it while
	 * laying out its title. Supply a deterministic, transient identity before
	 * rendering; the paused capture exits without persisting the savegame. */
	company->name_1 = STR_SV_UNNAMED;
	company->name = "OpenTTDRS Transport";
	company->president_name_1 = STR_SV_UNNAMED;
	company->president_name = "OpenTTDRS Manager";
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

bool OpenEconomyWindow(std::string_view id)
{
	if (id == "Finances" || id == "CompanyView") {
		Company *company = FirstCompany();
		if (company == nullptr) return false;
		PrepareCompanyIdentityForCapture(company);
		if (id == "Finances") ShowCompanyFinances(company->index);
		if (id == "CompanyView") ShowCompany(company->index);
		return true;
	}
	if (id == "GraphIncome") {
		ShowIncomeGraph();
		return true;
	}
	if (id == "GraphOperatingProfit") {
		ShowOperatingProfitGraph();
		return true;
	}
	if (id == "GraphCompanyValue") {
		ShowCompanyValueGraph();
		return true;
	}
	if (id == "CargoPaymentRates") {
		ShowCargoPaymentRates();
		return true;
	}
	if (id == "SubsidyList") {
		ShowSubsidiesList();
		return true;
	}
	if (id == "League") {
		ShowPerformanceLeagueTable();
		return true;
	}
	if (id == "NewsHistory") {
		ShowMessageHistory();
		return true;
	}
	if (id == "NewsSettings") {
		/* OpenTTD 15.3 has no standalone news-preferences window. Its equivalent
		 * configuration surface is the game-options window. Keep that distinction
		 * explicit in the manifest capture_route. */
		ShowGameOptions();
		return true;
	}
	return false;
}

Window *OpenAdvancedGameOptions()
{
	ShowGameOptions();
	Window *options = FindWindowByClass(WC_GAME_OPTIONS);
	if (options == nullptr) return nullptr;
	/* `settings_gui.cpp` keeps GameOptionsWindow private, but the public widget
	 * IDs and Window::OnClick let the driver select the real Advanced tab without
	 * hard-coded screen coordinates. */
	options->OnClick({0, 0}, WID_GO_TAB_ADVANCED, 1);
	return options;
}

bool OpenSettingsOrDialogWindow(std::string_view id)
{
	if (id == "NewGrf") {
		ShowNewGRFSettings(true, true, true, _grfconfig);
		return true;
	}
	if (id == "SoundMusic") {
		ShowMusicWindow();
		return true;
	}
	if (id == "AiSettings") {
		ShowAIConfigWindow();
		return true;
	}
	if (id == "Help") {
		ShowHelpWindow();
		return true;
	}
	if (id == "CheatWindow") {
		ShowCheatWindow();
		return true;
	}
	if (id == "DisplayOptions" || id == "PathfindingSettings" || id == "CargoDistSettings") {
		/* These are separate port surfaces. Their real 15.3 counterpart is the
		 * Advanced Game Options tab, not a fictional dedicated WindowClass. */
		return OpenAdvancedGameOptions() != nullptr;
	}
	if (id == "QueryString") {
		Window *parent = OpenAdvancedGameOptions();
		if (parent == nullptr) return false;
		ShowQueryString("OpenTTDRS", STR_CONFIG_SETTING_FILTER_TITLE, 50, parent, CS_ALPHANUMERAL, {});
		return true;
	}
	if (id == "OnScreenKeyboard") {
		Window *parent = OpenAdvancedGameOptions();
		if (parent == nullptr) return false;
		/* The Advanced filter is an actual QueryString owner, so OSK receives a
		 * valid text buffer instead of a synthetic, non-upstream stub. */
		ShowOnScreenKeyboard(parent, WID_GO_FILTER);
		return true;
	}
	if (id == "ErrorDialog") {
		/* Critical errors remain open through the deterministic settle frames. */
		ShowErrorMessage(GetEncodedString(STR_ERROR_CAN_T_BUILD_BRIDGE_HERE), {}, WL_CRITICAL);
		return true;
	}
	return false;
}

bool OpenCaptureWindow(std::string_view id)
{
	if (OpenConstructionPicker(id)) return true;
	if (OpenEconomyWindow(id)) return true;
	if (OpenSettingsOrDialogWindow(id)) return true;

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
