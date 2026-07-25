#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

usage() {
  echo "Uso: $0 <version> <target-rust> <plataforma> <tar.gz|zip>" >&2
}

if [[ $# -ne 4 ]]; then
  usage
  exit 2
fi

version="$1"
target="$2"
platform="$3"
archive_kind="$4"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  echo "Versión inválida: $version" >&2
  exit 2
fi
if [[ ! "$target" =~ ^[A-Za-z0-9_.-]+$ ]] || [[ ! "$platform" =~ ^[A-Za-z0-9_.-]+$ ]]; then
  echo "Target o plataforma inválidos." >&2
  exit 2
fi
if [[ "$archive_kind" != "tar.gz" && "$archive_kind" != "zip" ]]; then
  echo "Formato inválido: $archive_kind" >&2
  exit 2
fi

python_cmd="python3"
if ! command -v "$python_cmd" >/dev/null 2>&1; then
  python_cmd="python"
fi
manifest_version="$("$python_cmd" -c 'import pathlib, tomllib; print(tomllib.loads(pathlib.Path("Cargo.toml").read_text())["workspace"]["package"]["version"])')"
if [[ "$version" != "$manifest_version" ]]; then
  echo "La versión solicitada ($version) no coincide con Cargo.toml ($manifest_version)." >&2
  exit 1
fi

binary_dir="${ROOT}/target/${target}/release"
host_target="$(rustc -vV | sed -n 's/^host: //p')"
if [[ ! -d "$binary_dir" && "$target" == "$host_target" ]]; then
  binary_dir="${ROOT}/target/release"
fi
suffix=""
if [[ "$target" == *windows* ]]; then
  suffix=".exe"
fi

for binary in "openttdrs-client${suffix}" "openttdrs-dedicated${suffix}"; do
  if [[ ! -f "${binary_dir}/${binary}" ]]; then
    echo "Falta ${binary_dir}/${binary}; compilá ambos bins en release para ${target}." >&2
    exit 1
  fi
done

for required in \
  assets/music \
  assets/sounds \
  assets/opengfx/tiles \
  assets/opengfx/atlas \
  static/fonts/DejaVuSansMono.ttf \
  LICENSE \
  README.md \
  CHANGELOG.md \
  RELEASE_NOTES.md \
  THIRD_PARTY_ASSETS.md; do
  if [[ ! -e "${ROOT}/${required}" ]]; then
    echo "Falta ${required}; el paquete quedaría incompleto." >&2
    exit 1
  fi
done

package_name="openttdrs-${version}-${platform}"
dist_dir="${ROOT}/dist"
package_dir="${dist_dir}/${package_name}"
archive_path="${dist_dir}/${package_name}.${archive_kind}"

rm -rf "${package_dir}"
rm -f "${archive_path}" "${archive_path}.sha256"
mkdir -p "${package_dir}/assets/opengfx"

cp "${binary_dir}/openttdrs-client${suffix}" "${package_dir}/"
cp "${binary_dir}/openttdrs-dedicated${suffix}" "${package_dir}/"
cp -R "${ROOT}/assets/music" "${package_dir}/assets/"
cp -R "${ROOT}/assets/sounds" "${package_dir}/assets/"
cp -R "${ROOT}/assets/opengfx/tiles" "${package_dir}/assets/opengfx/"
cp -R "${ROOT}/assets/opengfx/atlas" "${package_dir}/assets/opengfx/"
cp -R "${ROOT}/static" "${package_dir}/"
cp \
  "${ROOT}/LICENSE" \
  "${ROOT}/README.md" \
  "${ROOT}/CHANGELOG.md" \
  "${ROOT}/RELEASE_NOTES.md" \
  "${ROOT}/THIRD_PARTY_ASSETS.md" \
  "${package_dir}/"

if [[ "$archive_kind" == "tar.gz" ]]; then
  tar -C "${dist_dir}" -czf "${archive_path}" "${package_name}"
else
  if ! command -v 7z >/dev/null 2>&1; then
    echo "Hace falta 7z para crear el ZIP." >&2
    exit 1
  fi
  (cd "${dist_dir}" && 7z a -bd -tzip "${archive_path}" "${package_name}" >/dev/null)
fi

(cd "${dist_dir}" && sha256sum "$(basename "${archive_path}")" > "$(basename "${archive_path}").sha256")
echo "Paquete: ${archive_path}"
echo "Checksum: ${archive_path}.sha256"
