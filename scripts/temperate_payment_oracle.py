#!/usr/bin/env python3
"""Genera o verifica el corpus V1-PAY con GetTransportedGoodsIncome nativo.

El harness extrae el cuerpo literal de ``GetTransportedGoodsIncome`` desde el
checkout OpenTTD fijado, lo compila con stubs mínimos de ``CargoSpec`` y emite
los 198 pagos Temperate del contrato. No calcula el lado de referencia con
Rust ni reescribe la fórmula de OpenTTD en Python.

Uso:
  python3 scripts/temperate_payment_oracle.py reference/openttd-upstream --check
  python3 scripts/temperate_payment_oracle.py reference/openttd-upstream --write
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/openttdrs-core/tests/fixtures/parity/temperate_payment_15_3.tsv"
PROVENANCE = ROOT / "crates/openttdrs-core/tests/fixtures/parity/temperate_payment_15_3.provenance.json"
REFERENCE_MANIFEST = ROOT / "docs/parity/openttd-reference.json"
ECONOMY_CPP = Path("src/economy.cpp")
CARGO_CONST_H = Path("src/table/cargo_const.h")
INFLATION_PAYMENT = 1 << 16
COUNTS = (0, 1, 100)
DISTANCES = (1, 32)
TRANSIT_DAYS = (0, 30, 100)

# Orden y labels que usa el catálogo Temperate original; el índice es el
# CargoType sintético del harness, no una tabla Rust.
TEMPERATE_CARGOS = (
    ("PASS", "CT_PASSENGERS"),
    ("COAL", "CT_COAL"),
    ("MAIL", "CT_MAIL"),
    ("OIL_", "CT_OIL"),
    ("LVST", "CT_LIVESTOCK"),
    ("GOOD", "CT_GOODS"),
    ("GRAI", "CT_GRAIN"),
    ("WOOD", "CT_WOOD"),
    ("IORE", "CT_IRON_ORE"),
    ("STEL", "CT_STEEL"),
    ("VALU", "CT_VALUABLES"),
)

ENTRY_RE = re.compile(
    r"^\s*MK\(\s*(?:0x[0-9A-Fa-f]+|\d+),\s*"
    r"(?P<label>CT_[A-Z_]+),\s*\d+,\s*\d+,\s*0x[0-9A-Fa-f]+,\s*"
    r"(?P<payment>\d+),\s*(?P<fast>\d+),\s*(?P<slow>\d+),",
    re.MULTILINE,
)


PREAMBLE = r"""
#include <algorithm>
#include <array>
#include <cstdint>
#include <iostream>

using uint = unsigned int;
using Money = std::int64_t;
using CargoType = std::uint8_t;

enum class CargoCallbackMask { ProfitCalc };

struct CargoCallbackMasks {
    bool Test(CargoCallbackMask) const { return false; }
};

struct CargoSpec {
    Money current_payment;
    int transit_periods[2];
    CargoCallbackMasks callback_mask;

    bool IsValid() const { return true; }
    static const CargoSpec *Get(CargoType type);
};

std::array<CargoSpec, 11> g_cargo_specs;

const CargoSpec *CargoSpec::Get(CargoType type)
{
    return &g_cargo_specs.at(type);
}

constexpr std::uint16_t CALLBACK_FAILED = 0xFFFF;
constexpr int CBID_CARGO_PROFIT_CALC = 0;

template <typename Target, typename Value>
constexpr Target ClampTo(Value value)
{
    return static_cast<Target>(value);
}

template <typename Value>
constexpr Value GB(Value value, int start, int count)
{
    return static_cast<Value>((value >> start) & ((Value{1} << count) - 1));
}

template <typename Value>
constexpr bool HasBit(Value value, int bit)
{
    return (value & (Value{1} << bit)) != 0;
}

inline std::uint16_t GetCargoCallback(int, int, std::uint32_t, const CargoSpec *)
{
    return CALLBACK_FAILED;
}

template <typename Left, typename Right>
constexpr Money BigMulS(Left left, Right right, int shift)
{
    return static_cast<Money>((static_cast<__int128>(left) * static_cast<__int128>(right)) >> shift);
}
"""


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def extract_function(source: str, signature: str) -> str:
    start = source.index(signature)
    body = source.index("{", start)
    depth = 1
    end = body + 1
    while depth:
        if end >= len(source):
            raise ValueError(f"función sin cierre: {signature}")
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[start:end] + "\n"


def reference_manifest() -> dict[str, object]:
    return json.loads(REFERENCE_MANIFEST.read_text(encoding="utf-8"))


def git_output(source: Path, *args: str) -> str:
    try:
        return subprocess.check_output(
            ["git", "-C", str(source), *args], text=True, stderr=subprocess.STDOUT
        ).strip()
    except subprocess.CalledProcessError as exc:
        raise RuntimeError(f"no se pudo verificar el checkout {source}: {exc.output.strip()}") from exc


def verify_source(source: Path, manifest: dict[str, object]) -> None:
    expected = str(manifest["commit"])
    actual = git_output(source, "rev-parse", "HEAD")
    if actual != expected:
        raise RuntimeError(f"OpenTTD HEAD={actual} no coincide con el pin {expected}")
    for relative in (ECONOMY_CPP, CARGO_CONST_H):
        if not (source / relative).is_file():
            raise RuntimeError(f"falta fuente nativa requerida: {source / relative}")
        for diff_args in (("diff", "--quiet", "--", str(relative)), ("diff", "--cached", "--quiet", "--", str(relative))):
            proc = subprocess.run(["git", "-C", str(source), *diff_args], check=False)
            if proc.returncode != 0:
                raise RuntimeError(f"la fuente nativa relevante tiene cambios locales: {relative}")


def parse_temperate_specs(cargo_const: Path) -> list[tuple[str, int, int, int]]:
    found: dict[str, tuple[int, int, int]] = {}
    for match in ENTRY_RE.finditer(cargo_const.read_text(encoding="utf-8")):
        label = match.group("label")
        found.setdefault(
            label,
            (
                int(match.group("payment")),
                int(match.group("fast")),
                int(match.group("slow")),
            ),
        )

    specs: list[tuple[str, int, int, int]] = []
    for cargo_label, native_label in TEMPERATE_CARGOS:
        try:
            payment, fast, slow = found[native_label]
        except KeyError as exc:
            raise RuntimeError(f"no se encontró {native_label} en {cargo_const}") from exc
        specs.append((cargo_label, payment, fast, slow))
    return specs


def harness_main(specs: list[tuple[str, int, int, int]]) -> str:
    names = ", ".join(f'"{name}"' for name, _, _, _ in specs)
    cargo_specs = ",\n        ".join(
        f"CargoSpec{{{payment}, {{{fast}, {slow}}}, CargoCallbackMasks{{}}}}"
        for _, payment, fast, slow in specs
    )
    return f"""
int main()
{{
    const char *cargo_names[] = {{{names}}};
    g_cargo_specs = std::array<CargoSpec, 11>{{
        {cargo_specs}
    }};

    std::cout << "cargo\\tcount\\tdistance\\ttransit_days\\tincome\\n";
    for (int cargo = 0; cargo < 11; ++cargo) {{
        for (uint count : {{{", ".join(str(value) for value in COUNTS)}}}) {{
            for (uint distance : {{{", ".join(str(value) for value in DISTANCES)}}}) {{
                for (uint16_t transit_days : {{{", ".join(str(value) for value in TRANSIT_DAYS)}}}) {{
                    std::cout << cargo_names[cargo] << '\\t' << count << '\\t' << distance << '\\t'
                              << transit_days << '\\t'
                              << GetTransportedGoodsIncome(count, distance, transit_days, static_cast<CargoType>(cargo))
                              << '\\n';
                }}
            }}
        }}
    }}
}}
"""


def native_table(source: Path) -> str:
    economy = (source / ECONOMY_CPP).read_text(encoding="utf-8")
    literal_function = extract_function(
        economy,
        "Money GetTransportedGoodsIncome(uint num_pieces, uint dist, uint16_t transit_periods, CargoType cargo_type)",
    )
    specs = parse_temperate_specs(source / CARGO_CONST_H)
    with tempfile.TemporaryDirectory(prefix="temperate-payment-oracle-") as directory:
        temp = Path(directory)
        source_file = temp / "oracle.cpp"
        binary = temp / "oracle"
        source_file.write_text(PREAMBLE + literal_function + harness_main(specs), encoding="utf-8")
        subprocess.run(["c++", "-std=c++20", str(source_file), "-o", str(binary)], check=True)
        return subprocess.check_output([str(binary)], text=True)


def fixture_case_count(table: str) -> int:
    lines = table.splitlines()
    if not lines or lines[0] != "cargo\tcount\tdistance\ttransit_days\tincome":
        raise RuntimeError("la tabla nativa no tiene el encabezado V1-PAY")
    expected = len(TEMPERATE_CARGOS) * len(COUNTS) * len(DISTANCES) * len(TRANSIT_DAYS)
    if len(lines) - 1 != expected:
        raise RuntimeError(f"la tabla nativa tiene {len(lines) - 1} casos; se esperaban {expected}")
    return expected


def expected_provenance(source: Path, manifest: dict[str, object], table: str) -> dict[str, object]:
    return {
        "schema_version": 1,
        "contract": "V1-PAY",
        "generator": "scripts/temperate_payment_oracle.py",
        "oracle": {
            "openttd_tag": manifest["tag"],
            "openttd_commit": git_output(source, "rev-parse", "HEAD"),
            "function": "GetTransportedGoodsIncome",
            "execution": "literal C++ body extracted and compiled with CargoSpec stubs",
            "source_sha256": {
                str(ECONOMY_CPP): sha256_file(source / ECONOMY_CPP),
                str(CARGO_CONST_H): sha256_file(source / CARGO_CONST_H),
            },
        },
        "parameters": {
            "inflation_payment": INFLATION_PAYMENT,
            "counts": list(COUNTS),
            "distances": list(DISTANCES),
            "transit_days": list(TRANSIT_DAYS),
            "newgrf": False,
            "climate": "Temperate",
            "cargo_labels": [label for label, _ in TEMPERATE_CARGOS],
        },
        "fixture": {
            "path": str(FIXTURE.relative_to(ROOT)),
            "sha256": sha256_bytes(table.encode("utf-8")),
            "cases": fixture_case_count(table),
        },
    }


def first_difference(expected: str, actual: str) -> str:
    expected_lines = expected.splitlines()
    actual_lines = actual.splitlines()
    for index, (left, right) in enumerate(zip(expected_lines, actual_lines), start=1):
        if left != right:
            return f"línea {index}: esperado {left!r}; actual {right!r}"
    return f"cantidad de líneas: esperado {len(expected_lines)}, actual {len(actual_lines)}"


def check_fixture(table: str, provenance: dict[str, object]) -> None:
    if not FIXTURE.is_file():
        raise RuntimeError(f"falta fixture versionada: {FIXTURE}")
    current = FIXTURE.read_text(encoding="utf-8")
    if current != table:
        raise RuntimeError(f"DRIFT V1-PAY: {first_difference(table, current)}")
    if not PROVENANCE.is_file():
        raise RuntimeError(f"falta procedencia versionada: {PROVENANCE}")
    current_provenance = json.loads(PROVENANCE.read_text(encoding="utf-8"))
    if current_provenance != provenance:
        raise RuntimeError("DRIFT V1-PAY: la procedencia/hash no coincide con la tabla nativa")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "source",
        nargs="?",
        type=Path,
        default=ROOT / "reference/openttd-upstream",
        help="checkout OpenTTD fijado (default: reference/openttd-upstream)",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="comparar contra fixture/procedencia versionadas")
    mode.add_argument("--write", action="store_true", help="actualizar fixture/procedencia desde el oracle nativo")
    args = parser.parse_args(argv)

    try:
        manifest = reference_manifest()
        verify_source(args.source, manifest)
        table = native_table(args.source)
        provenance = expected_provenance(args.source, manifest, table)
        if args.check:
            check_fixture(table, provenance)
            print(f"OK: V1-PAY {provenance['fixture']['cases']} casos nativos; hash {provenance['fixture']['sha256']}")
        elif args.write:
            FIXTURE.write_text(table, encoding="utf-8")
            PROVENANCE.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            print(f"Escritos {FIXTURE.relative_to(ROOT)} y {PROVENANCE.relative_to(ROOT)}")
        else:
            print(table, end="")
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"FAIL V1-PAY: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
