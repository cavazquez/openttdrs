#!/usr/bin/env bash
# Lectura del manifiesto de referencia OpenTTD (#109).
# shellcheck shell=bash

openttd_manifest_path() {
  local root="$1"
  echo "${root}/docs/parity/openttd-reference.json"
}

# Imprime un campo del manifiesto (python3; sin dependencia de jq).
openttd_manifest_get() {
  local root="$1"
  local key="$2"
  local path
  path="$(openttd_manifest_path "$root")"
  if [[ ! -f "$path" ]]; then
    echo "error: falta manifiesto ${path}" >&2
    return 1
  fi
  python3 - "$path" "$key" <<'PY'
import json, sys
path, key = sys.argv[1], sys.argv[2]
data = json.load(open(path, encoding="utf-8"))
if key not in data:
    raise SystemExit(f"campo ausente en manifiesto: {key}")
print(data[key])
PY
}

openttd_manifest_summary() {
  local root="$1"
  local path commit tag url pinned license
  path="$(openttd_manifest_path "$root")"
  commit="$(openttd_manifest_get "$root" commit)"
  tag="$(openttd_manifest_get "$root" tag)"
  url="$(openttd_manifest_get "$root" url)"
  pinned="$(openttd_manifest_get "$root" pinned_at)"
  license="$(openttd_manifest_get "$root" license_spdx)"
  echo "OpenTTD reference: tag=${tag} commit=${commit}"
  echo "  url=${url}"
  echo "  pinned_at=${pinned} license=${license}"
  echo "  manifest=${path}"
}
