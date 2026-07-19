/*
 * Snapshot export for openttdrs parity (#110).
 * Applied onto OpenTTD @ docs/parity/openttd-reference.json (tag 15.3).
 */

#ifndef OPENTTDRS_SNAPSHOT_EXPORT_H
#define OPENTTDRS_SNAPSHOT_EXPORT_H

#include <string>

/**
 * If OPENTTDRS_SNAPSHOT_OUT is set, write a JSON snapshot compatible with
 * openttdrs `snapshot_dumper` hard fields (see docs/parity/SNAPSHOT_SCHEMA.md).
 * @param source_path Path of the loaded save (for metadata only).
 * @return true on success or when the env var is unset; false on I/O error.
 */
bool OpenttdrsMaybeExportSnapshot(const std::string &source_path);

/**
 * Arms an optional PBS JSONL exporter once the requested save completed loading.
 *
 * The exporter is enabled only when OPENTTDRS_PBS_TRACE_OUT is set. Its rows
 * are emitted by OpenttdrsMaybeExportPbsTraceTick() after each game tick.
 */
void OpenttdrsMaybeStartPbsTrace(const std::string &source_path);

/** Emits one post-tick PBS row and exits after the configured number of rows. */
void OpenttdrsMaybeExportPbsTraceTick();

#endif /* OPENTTDRS_SNAPSHOT_EXPORT_H */
