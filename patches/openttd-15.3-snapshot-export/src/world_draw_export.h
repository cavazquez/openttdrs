/*
 * Opt-in trace of the real OpenTTD tile draw calls used by openttdrs #307.
 */

#ifndef OPENTTDRS_WORLD_DRAW_EXPORT_H
#define OPENTTDRS_WORLD_DRAW_EXPORT_H

#include <cstdint>
#include <string>

/** Inclusive map-coordinate bounds requested for the current capture. */
struct OpenttdrsWorldDrawBounds {
	uint32_t begin_x = 0;
	uint32_t begin_y = 0;
	uint32_t end_x = 0; /* exclusive */
	uint32_t end_y = 0; /* exclusive */
};

/**
 * Reads OPENTTDRS_WORLD_DRAW_OUT and opens a `world-draw` v1 stream after a
 * save has loaded. It intentionally does not rasterize or create a window.
 */
bool OpenttdrsMaybeStartWorldDraw(const std::string &source_path);

bool OpenttdrsWorldDrawCaptureActive();
bool OpenttdrsWorldDrawCaptureBounds(OpenttdrsWorldDrawBounds &bounds);

/**
 * `OPENTTDRS_WORLD_SORT_OUT` habilita un segundo stream opt-in con el orden
 * final que produce `ViewportSortParentSprites`. Requiere también
 * `OPENTTDRS_WORLD_DRAW_OUT`: el stream de draw-procs conserva el vínculo
 * `parent_id` que hace diagnosticable cada entrada ordenada.
 */
bool OpenttdrsWorldDrawFinalSortRequested();

/**
 * Abre una tesela del trace e incluye tanto su pendiente cruda como la
 * superficie efectiva que usa `GetFoundationSlope` para decidir muros.
 * Las alturas están en unidades de tile, no en píxeles de viewport.
 */
void OpenttdrsWorldDrawBeginTile(
	uint32_t x,
	uint32_t y,
	uint8_t tile_type,
	uint8_t tileh,
	uint32_t base_z,
	uint8_t foundation_tileh,
	uint32_t foundation_base_z
);
void OpenttdrsWorldDrawEndTile();

/**
 * Expone la decisión interna de `DrawFoundation`: el tipo de fundación y las
 * comparaciones de sus bordes NO/NE que escogen el bloque de sprites. Las
 * ocho alturas posteriores son las cuatro comparaciones exactas que hizo
 * OpenTTD (en píxeles de altura), para poder contrastarlas con el cliente.
 *
 * `foundation_base_z` está en unidades de tile, igual que el campo de tesela
 * de este contrato (no en píxeles de viewport).
 */
void OpenttdrsWorldDrawRecordFoundation(
	uint8_t foundation,
	uint8_t foundation_tileh,
	uint32_t foundation_base_z,
	uint8_t sprite_block,
	bool has_nw,
	bool has_ne,
	int32_t nw_w_here,
	int32_t nw_n_here,
	int32_t nw_w_neighbour,
	int32_t nw_n_neighbour,
	int32_t ne_e_here,
	int32_t ne_n_here,
	int32_t ne_e_neighbour,
	int32_t ne_n_neighbour
);

/**
 * `offset_x/y` conserva el desplazamiento final de pantalla que recibe
 * `AddTileSpriteToDraw`. Es relevante para `TILE_SEQ_GROUND`: el mundo sigue
 * siendo el origen de la tesela y el desplazamiento queda separado en la
 * traza, igual que para los sprites sortable.
 */
void OpenttdrsWorldDrawRecordTileSprite(
	uint32_t image,
	uint32_t palette,
	int32_t x,
	int32_t y,
	int32_t z,
	int32_t offset_x,
	int32_t offset_y
);
void OpenttdrsWorldDrawRecordSortable(
	uint32_t image,
	uint32_t palette,
	int32_t x,
	int32_t y,
	int32_t z,
	int32_t origin_x,
	int32_t origin_y,
	int32_t origin_z,
	int32_t extent_x,
	int32_t extent_y,
	int32_t extent_z,
	int32_t offset_x,
	int32_t offset_y,
	int32_t offset_z,
	bool transparent,
	uint8_t combine_mode
);
void OpenttdrsWorldDrawRecordChild(
	uint32_t image,
	uint32_t palette,
	int32_t x,
	int32_t y,
	bool transparent,
	bool scale,
	bool relative
);
void OpenttdrsWorldDrawRecordCombineStart();
void OpenttdrsWorldDrawRecordCombineEnd();

/**
 * Emite el resultado final del sorter de padres. `parent_id` es el índice
 * estable en `parent_sprites_to_draw`, compartido con los draws del stream
 * `world-draw`; `final_ordinal` es la posición efectiva de pintado.
 */
void OpenttdrsWorldDrawBeginFinalSort(uint64_t parent_count, uint64_t child_count);
void OpenttdrsWorldDrawRecordFinalParent(
	uint64_t final_ordinal,
	uint64_t parent_id,
	uint32_t image,
	uint32_t palette,
	int32_t screen_x,
	int32_t screen_y,
	int32_t left,
	int32_t top,
	int32_t xmin,
	int32_t ymin,
	int32_t zmin,
	int32_t xmax,
	int32_t ymax,
	int32_t zmax,
	int32_t first_child
);
void OpenttdrsWorldDrawRecordFinalChild(
	uint64_t final_parent_ordinal,
	uint64_t parent_id,
	uint64_t child_ordinal,
	int32_t child_index,
	uint32_t image,
	uint32_t palette,
	int32_t x,
	int32_t y,
	bool relative,
	int32_t next
);
void OpenttdrsWorldDrawFinishFinalSort();

/** Flushes the trace and requests termination of the dedicated reference run. */
bool OpenttdrsFinishWorldDraw();

#endif /* OPENTTDRS_WORLD_DRAW_EXPORT_H */
