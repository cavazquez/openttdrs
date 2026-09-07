/*
 * Opt-in viewport screenshot oracle for openttdrs renderer parity.
 *
 * This lives beside the headless draw-call exporter, but deliberately uses
 * OpenTTD's normal screenshot pipeline. It is only activated by an output
 * path supplied through OPENTTDRS_WORLD_SCREENSHOT_OUT.
 */

#ifndef OPENTTDRS_WORLD_SCREENSHOT_EXPORT_H
#define OPENTTDRS_WORLD_SCREENSHOT_EXPORT_H

/**
 * Schedules one map screenshot after loading a game, copies it to
 * OPENTTDRS_WORLD_SCREENSHOT_OUT, and then exits OpenTTD. Optional variables:
 *
 * - OPENTTDRS_WORLD_SCREENSHOT_CENTER=x,y
 * - OPENTTDRS_WORLD_SCREENSHOT_RES=widthxheight
 * - OPENTTDRS_WORLD_SCREENSHOT_SCALE=0.25|0.5|1|2|4|8 (convención de la
 *   escala ortográfica de openttdrs; por defecto 1 / ZoomLevel::Normal)
 * - OPENTTDRS_WORLD_SCREENSHOT_CLEAN=1 (pausa y oculta rótulos/animación)
 * - OPENTTDRS_WORLD_SCREENSHOT_SORT_OUT=/ruta/trace.jsonl (opcional;
 *   conserva los parents finales de cada segmento de raster, después del
 *   sorter real del viewport)
 * - OPENTTDRS_WORLD_SCREENSHOT_MIN_CALL=N (por defecto 2; omite la partida
 *   temporal que el dedicado carga antes del .sav indicado con -g)
 */
bool OpenttdrsMaybeCaptureWorldScreenshot();

/** `true` while the opt-in clean raster capture excludes dynamic vehicles. */
bool OpenttdrsWorldScreenshotHideVehicles();

/**
 * Abre/cierra el stream opt-in del orden real de parents de la captura
 * raster. El exportador lo arma justo antes de encolar `MakeScreenshotAtZoom`;
 * por eso no mezcla redraws normales de la ventana con los segmentos del PNG.
 */
bool OpenttdrsWorldScreenshotStartSortTrace(uint32_t width, uint32_t height, int zoom);
bool OpenttdrsWorldScreenshotFinishSortTrace();

/** Devuelve si el `ViewportDoDraw` actual pertenece a esa captura exacta. */
bool OpenttdrsWorldScreenshotSortTraceMatches(
	int viewport_left, int viewport_top, int viewport_width, int viewport_height, int zoom
);

/** Serialización de un segmento ya ordenado por `ViewportSortParentSprites`. */
void OpenttdrsWorldScreenshotBeginSortSegment(
	int virtual_left, int virtual_top, int virtual_width, int virtual_height,
	uint64_t parent_count, uint64_t child_count
);
void OpenttdrsWorldScreenshotRecordSortParent(
	uint64_t final_ordinal, uint64_t parent_id,
	uint32_t image, uint32_t palette,
	int screen_x, int screen_y, int left, int top,
	int xmin, int ymin, int zmin, int xmax, int ymax, int zmax, int first_child
);
void OpenttdrsWorldScreenshotFinishSortSegment();

#endif /* OPENTTDRS_WORLD_SCREENSHOT_EXPORT_H */
