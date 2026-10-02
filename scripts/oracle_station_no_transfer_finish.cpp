
#include <algorithm>
#include <cassert>
#include <cstdint>
#include <iostream>
#include <list>
#include "native-enums.inc"
struct StationID { uint16_t value=0; static constexpr StationID Invalid(){return {65535};} bool operator==(const StationID&) const = default; };
enum class VehState { Crashed };
enum class VehicleFlag { LoadingFinished };
enum class VehicleRailFlag { LeavingStation };
enum class StationRandomTrigger { VehicleDeparts };
enum class StationAnimationTrigger { VehicleDeparts };
struct Vehicle;
struct Flags { bool value=false;bool Test(VehState) const {return value;}bool Test(VehicleFlag)const{return value;}void Set(VehicleRailFlag){value=true;}void Set(VehicleFlag,bool flag){value=flag;} };
struct NonStop { bool any=false;bool Any()const{return any;} };
struct Order {
 OrderType type=OT_LOADING;
 int timed_wait=0;OrderType GetType()const{return type;}int GetTimetabledWait()const{return timed_wait;}StationID GetDestination()const{return {0};}
 bool nonstop=false,can_leave=false;
 OrderLoadType load=OrderLoadType::LoadIfPossible;
 OrderUnloadType unload=OrderUnloadType::UnloadIfPossible;
 bool IsType(OrderType other)const{return type==other;}
 NonStop GetNonStopType()const{return {nonstop};}
 OrderLoadType GetLoadType()const{return load;}
 OrderUnloadType GetUnloadType()const{return unload;}
 bool IsFullLoadOrder()const{return load==OrderLoadType::FullLoad||load==OrderLoadType::FullLoadAny;}
 bool CanLeaveWithCargo(bool)const{return can_leave;}
 void MakeLeaveStation(){type=OT_LEAVESTATION;}
};
struct Station {
 std::list<Vehicle*> loading_vehicles;
 static Station *Get(StationID){static Station st;return &st;}
};
struct TimerGameTick {using Ticks=int;static constexpr uint32_t counter=100;};
unsigned link_refreshes=0;
struct LinkRefresher {static void Run(Vehicle*){}static void Run(Vehicle*,bool,bool){++link_refreshes;}};
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
struct Cargo {unsigned stored=0;unsigned StoredCount()const{return stored;}};
struct Vehicle {
 Cargo cargo;unsigned cargo_cap=0;unsigned cargo_type=0;
 Order current_order;
 int *cargo_payment=nullptr;
 VehicleType type=VEH_TRAIN;
 Flags vehstatus,flags,vehicle_flags;
 int current_order_time=0,lateness_counter=0;unsigned cur_implicit_order_index=0;
 Order real_order;const Order *GetOrder(unsigned)const{return &real_order;}
 void PlayLeaveStationSound(){}void IncrementImplicitOrderIndex(){++cur_implicit_order_index;}void HandleLoading(bool);
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
#include "native-handle-loading.inc"

using CargoTypes=uint64_t;
enum class CargoClass{Passengers};
bool IsCargoInClass(unsigned,CargoClass){return false;}
void UpdateLoadUnloadTicks(Vehicle*,Station*,int ticks){assert(ticks==20);}
void NativeNoTransferCompletion(Vehicle *front,Station *st,CargoTypes cargo_full,CargoTypes cargo_not_full){
#include "native-no-transfer-finish.inc"
}
int main(){
 std::cout<<"type,load,unload,capacity,cargo,input_progress,input_subspeed,finished,speed,progress,subspeed,order,index,registered,link_refreshes\n";
 for(auto type:{VEH_TRAIN,VEH_ROAD})
 for(auto load:{OrderLoadType::LoadIfPossible,OrderLoadType::FullLoad,OrderLoadType::FullLoadAny,OrderLoadType::NoLoad})
 for(auto unload:{OrderUnloadType::UnloadIfPossible,OrderUnloadType::Unload,OrderUnloadType::Transfer,OrderUnloadType::NoUnload})
 for(unsigned capacity:{0U,40U})
 for(unsigned cargo:{0U,1U,39U,40U})
 for(unsigned progress:{0U,1U,63U,95U,127U,191U,255U})
 for(unsigned subspeed:{0U,99U,255U}){
  Vehicle v;v.type=type;v.current_order.load=load;v.current_order.unload=unload;v.cargo_cap=capacity;v.cargo.stored=cargo;v.progress=progress;v.subspeed=subspeed;v.real_order.type=OT_GOTO_STATION;
  auto *st=Station::Get(v.last_station_visited);st->loading_vehicles.clear();st->loading_vehicles.push_back(&v);link_refreshes=0;
  CargoTypes cargo_full=capacity!=0&&cargo>=capacity?1:0;CargoTypes cargo_not_full=capacity!=0&&cargo<capacity?1:0;
  NativeNoTransferCompletion(&v,st,cargo_full,cargo_not_full);bool finished=v.vehicle_flags.value;v.HandleLoading(false);
  std::cout<<unsigned(type)<<','<<unsigned(load)<<','<<unsigned(unload)<<','<<capacity<<','<<cargo<<','<<progress<<','<<subspeed<<','<<finished<<','<<v.cur_speed<<','<<unsigned(v.progress)<<','<<unsigned(v.subspeed)<<','<<unsigned(v.current_order.type)<<','<<v.cur_implicit_order_index<<','<<!st->loading_vehicles.empty()<<','<<link_refreshes<<'\n';
 }
}
