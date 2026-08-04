#!/usr/bin/env bash
# Comprueba que los binarios Linux no requieran un GLIBC posterior al baseline.
set -euo pipefail

usage() {
  echo "Uso: $0 <baseline-glibc> <cliente> <dedicated>" >&2
}

if [[ $# -ne 3 ]]; then
  usage
  exit 2
fi

baseline="$1"
client="$2"
dedicated="$3"
if [[ ! "$baseline" =~ ^[0-9]+\.[0-9]+$ ]]; then
  echo "Baseline GLIBC inválido: $baseline" >&2
  exit 2
fi
for binary in "$client" "$dedicated"; do
  if [[ ! -f "$binary" ]]; then
    echo "No existe binario Linux: $binary" >&2
    exit 2
  fi
done

workdir="$(mktemp -d)"
cleanup() {
  rm -rf "$workdir"
}
trap cleanup EXIT

versions="${workdir}/glibc-versions.txt"
readelf --version-info "$client" >"${workdir}/client.readelf"
readelf --version-info "$dedicated" >"${workdir}/dedicated.readelf"
grep -h -oE 'GLIBC_[0-9]+(\.[0-9]+)+' "${workdir}"/*.readelf >"$versions" || true
if [[ ! -s "$versions" ]]; then
  echo "No se encontraron versiones GLIBC en los binarios." >&2
  exit 1
fi
sort -Vu "$versions" -o "$versions"
required="$(tail -n 1 "$versions")"
required="${required#GLIBC_}"

IFS=. read -r required_major required_minor <<<"$required"
IFS=. read -r baseline_major baseline_minor <<<"$baseline"
if (( required_major > baseline_major )) || \
  (( required_major == baseline_major && required_minor > baseline_minor )); then
  echo "Los binarios requieren GLIBC_${required}; el baseline declarado es GLIBC_${baseline}." >&2
  exit 1
fi

echo "ABI Linux OK: máximo GLIBC_${required}; baseline GLIBC_${baseline}."
