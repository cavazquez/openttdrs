
#include <cassert>
#include <cstdint>
#include <iostream>
#include <list>
#include "native-enums.inc"
struct StationID { uint16_t value=0; static constexpr StationID Invalid(){return {65535};} bool operator==(const StationID&) const = default; };
enum class VehState { Crashed };
enum class VehicleRailFlag { LeavingStation };
enum class StationRandomTrigger { VehicleDeparts };
enum class StationAnimationTrigger { VehicleDeparts };
struct Vehicle;
struct Flags { bool value=false;bool Test(VehState) const {return value;}void Set(VehicleRailFlag){value=true;} };
struct NonStop { bool any=false;bool Any()const{return any;} };
struct Order {
 OrderType type=OT_LOADING;
 bool nonstop=false,can_leave=false;
 OrderLoadType load=OrderLoadType::LoadIfPossible;
 OrderUnloadType unload=OrderUnloadType::UnloadIfPossible;
 bool IsType(OrderType other)const{return type==other;}
 NonStop GetNonStopType()const{return {nonstop};}
 OrderLoadType GetLoadType()const{return load;}
 OrderUnloadType GetUnloadType()const{return unload;}
 bool CanLeaveWithCargo(bool)const{return can_leave;}
 void MakeLeaveStation(){type=OT_LEAVESTATION;}
};
struct Station {
 std::list<Vehicle*> loading_vehicles;
 static Station *Get(StationID){static Station st;return &st;}
};
struct TimerGameTick {static constexpr uint32_t counter=100;};
struct LinkRefresher {static void Run(Vehicle*){}};
constexpr int MP_STATION=0;
void UpdateVehicleTimetable(Vehicle*,bool){}
void HideFillingPercent(int*){}
int CalcPercentVehicleFilled(Vehicle*,void*){return 41;}
int trip_occupancy=0;
bool IsTileType(int,int){return true;}
bool IsStationRoadStopTile(int){return true;}
void TriggerStationRandomisation(Station*,int,StationRandomTrigger){}
void TriggerStationAnimation(Station*,int,StationAnimationTrigger){}
void TriggerRoadStopRandomisation(Station*,int,StationRandomTrigger){}
void TriggerRoadStopAnimation(Station*,int,StationAnimationTrigger){}
struct Vehicle {
 Order current_order;
 int *cargo_payment=nullptr;
 VehicleType type=VEH_TRAIN;
 Flags vehstatus,flags;
 StationID last_loading_station=StationID::Invalid(),last_station_visited{0};
 uint32_t last_loading_tick=0;
 int fill_percent_te_id=0,tile=0;
 uint16_t cur_speed=0;
 uint8_t subspeed=0,progress=0;
 void ResetRefitCaps(){}
 void CancelReservation(StationID,Station*){}
 void MarkDirty(){}
 static Vehicle *From(Vehicle *v){return v;}
 void LeaveStation();
};
using Train=Vehicle;
#include "native-leave-station.inc"
int main(){
 std::cout<<"type,crashed,nonstop,no_load,no_unload,can_leave,input_progress,input_subspeed,speed,progress,subspeed,order,registered,leaving\n";
 for(auto type:{VEH_TRAIN,VEH_ROAD})
 for(bool crashed:{false,true})
 for(bool nonstop:{false,true})
 for(bool no_load:{false,true})
 for(bool no_unload:{false,true})
 for(bool can_leave:{false,true})
 for(unsigned progress:{0U,1U,63U,95U,127U,191U,255U})
 for(unsigned subspeed:{0U,99U,255U}){
  Vehicle v;v.type=type;v.vehstatus.value=crashed;v.current_order.nonstop=nonstop;
  v.current_order.load=no_load?OrderLoadType::NoLoad:OrderLoadType::LoadIfPossible;
  v.current_order.unload=no_unload?OrderUnloadType::NoUnload:OrderUnloadType::UnloadIfPossible;
  v.current_order.can_leave=can_leave;v.progress=progress;v.subspeed=subspeed;
  auto *st=Station::Get(v.last_station_visited);assert(st->loading_vehicles.empty());st->loading_vehicles.push_back(&v);
  v.LeaveStation();
  std::cout<<unsigned(type)<<','<<crashed<<','<<nonstop<<','<<no_load<<','<<no_unload<<','<<can_leave<<','<<progress<<','<<subspeed<<','<<v.cur_speed<<','<<unsigned(v.progress)<<','<<unsigned(v.subspeed)<<','<<unsigned(v.current_order.type)<<','<<!st->loading_vehicles.empty()<<','<<v.flags.value<<'\n';
 }
}
