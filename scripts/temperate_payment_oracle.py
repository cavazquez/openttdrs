#!/usr/bin/env python3
"""Genera o verifica los corpus V1-PAY/V1-COAL-TRANSFER nativos.

El harness extrae el cuerpo literal de ``GetTransportedGoodsIncome`` desde el
checkout OpenTTD fijado, lo compila con stubs mínimos de ``CargoSpec`` y emite
los 198 pagos Temperate del contrato y la traza acotada de carbón con dos
tramos. No calcula el lado de referencia con Rust ni reescribe la fórmula de
OpenTTD en Python.

Uso:
  python3 scripts/temperate_payment_oracle.py reference/openttd-upstream --check
  python3 scripts/temperate_payment_oracle.py reference/openttd-upstream --write
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "crates/openttdrs-core/tests/fixtures/parity/temperate_payment_15_3.tsv"
PROVENANCE = ROOT / "crates/openttdrs-core/tests/fixtures/parity/temperate_payment_15_3.provenance.json"
TRANSFER_FIXTURE = ROOT / "crates/openttdrs-core/tests/fixtures/parity/coal_transfer_15_3.tsv"
TRANSFER_PROVENANCE = (
    ROOT / "crates/openttdrs-core/tests/fixtures/parity/coal_transfer_15_3.provenance.json"
)
REFERENCE_MANIFEST = ROOT / "docs/parity/openttd-reference.json"
ECONOMY_CPP = Path("src/economy.cpp")
CARGO_CONST_H = Path("src/table/cargo_const.h")
INFLATION_PAYMENT = 1 << 16
COUNTS = (0, 1, 100)
DISTANCES = (1, 32)
TRANSIT_DAYS = (0, 30, 100)
TRANSFER_TRACE_CASES = (
    ("transfer", "COAL", 4, 20, 7),
    ("final", "COAL", 4, 40, 24),
)


def default_source() -> Path:
    """Return the pinned checkout, optionally selected for a local isolated run."""
    configured = os.environ.get("OPENTTDRS_PAYMENT_ORACLE_SOURCE")
    if not configured:
        return ROOT / "reference/openttd-upstream"
    source = Path(configured)
    return source if source.is_absolute() else ROOT / source

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


def transfer_trace_harness_main(specs: list[tuple[str, int, int, int]]) -> str:
    cargo_indices = {name: index for index, (name, _, _, _) in enumerate(specs)}
    rows = ",\n        ".join(
        "TraceCase{"
        f'"{phase}", static_cast<CargoType>({cargo_indices[cargo]}), {count}, {distance}, {transit}'
        "}"
        for phase, cargo, count, distance, transit in TRANSFER_TRACE_CASES
    )
    cargo_specs = ",\n        ".join(
        f"CargoSpec{{{payment}, {{{fast}, {slow}}}, CargoCallbackMasks{{}}}}"
        for _, payment, fast, slow in specs
    )
    return f"""
struct TraceCase {{
    const char *phase;
    CargoType cargo;
    uint count;
    uint distance;
    uint16_t transit_days;
}};

int main()
{{
    g_cargo_specs = std::array<CargoSpec, 11>{{
        {cargo_specs}
    }};
    const std::array<TraceCase, {len(TRANSFER_TRACE_CASES)}> cases{{{{
        {rows}
    }}}};

    std::cout << "phase\\tcargo\\tcount\\tdistance\\ttransit_days\\tincome\\n";
    for (const TraceCase &row : cases) {{
        std::cout << row.phase << '\\t' << "COAL" << '\\t' << row.count << '\\t'
                  << row.distance << '\\t' << row.transit_days << '\\t'
                  << GetTransportedGoodsIncome(row.count, row.distance, row.transit_days, row.cargo)
                  << '\\n';
    }}
}}
"""


def run_native_harness(literal_function: str, harness: str) -> str:
    with tempfile.TemporaryDirectory(prefix="temperate-payment-oracle-") as directory:
        temp = Path(directory)
        source_file = temp / "oracle.cpp"
        binary = temp / "oracle"
        source_file.write_text(PREAMBLE + literal_function + harness, encoding="utf-8")
        subprocess.run(["c++", "-std=c++20", str(source_file), "-o", str(binary)], check=True)
        return subprocess.check_output([str(binary)], text=True)


def native_harness_inputs(source: Path) -> tuple[str, list[tuple[str, int, int, int]]]:
    economy = (source / ECONOMY_CPP).read_text(encoding="utf-8")
    literal_function = extract_function(
        economy,
        "Money GetTransportedGoodsIncome(uint num_pieces, uint dist, uint16_t transit_periods, CargoType cargo_type)",
    )
    return literal_function, parse_temperate_specs(source / CARGO_CONST_H)


def native_table(source: Path) -> str:
    literal_function, specs = native_harness_inputs(source)
    return run_native_harness(literal_function, harness_main(specs))


def native_transfer_trace(source: Path) -> str:
    literal_function, specs = native_harness_inputs(source)
    return run_native_harness(literal_function, transfer_trace_harness_main(specs))


def fixture_case_count(table: str) -> int:
    lines = table.splitlines()
    if not lines or lines[0] != "cargo\tcount\tdistance\ttransit_days\tincome":
        raise RuntimeError("la tabla nativa no tiene el encabezado V1-PAY")
    expected = len(TEMPERATE_CARGOS) * len(COUNTS) * len(DISTANCES) * len(TRANSIT_DAYS)
    if len(lines) - 1 != expected:
        raise RuntimeError(f"la tabla nativa tiene {len(lines) - 1} casos; se esperaban {expected}")
    return expected


def transfer_trace_case_count(trace: str) -> int:
    lines = trace.splitlines()
    if not lines or lines[0] != "phase\tcargo\tcount\tdistance\ttransit_days\tincome":
        raise RuntimeError("la traza nativa no tiene el encabezado V1-COAL-TRANSFER")
    if len(lines) - 1 != len(TRANSFER_TRACE_CASES):
        raise RuntimeError(
            f"la traza nativa tiene {len(lines) - 1} casos; se esperaban {len(TRANSFER_TRACE_CASES)}"
        )
    return len(TRANSFER_TRACE_CASES)


def oracle_provenance(source: Path, manifest: dict[str, object]) -> dict[str, object]:
    return {
        "openttd_tag": manifest["tag"],
        "openttd_commit": git_output(source, "rev-parse", "HEAD"),
        "function": "GetTransportedGoodsIncome",
        "execution": "literal C++ body extracted and compiled with CargoSpec stubs",
        "source_sha256": {
            str(ECONOMY_CPP): sha256_file(source / ECONOMY_CPP),
            str(CARGO_CONST_H): sha256_file(source / CARGO_CONST_H),
        },
    }


def expected_provenance(source: Path, manifest: dict[str, object], table: str) -> dict[str, object]:
    return {
        "schema_version": 1,
        "contract": "V1-PAY",
        "generator": "scripts/temperate_payment_oracle.py",
        "oracle": oracle_provenance(source, manifest),
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


def expected_transfer_provenance(
    source: Path, manifest: dict[str, object], trace: str
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "contract": "V1-COAL-TRANSFER",
        "generator": "scripts/temperate_payment_oracle.py",
        "oracle": oracle_provenance(source, manifest),
        "parameters": {
            "inflation_payment": INFLATION_PAYMENT,
            "newgrf": False,
            "climate": "Temperate",
            "cases": [
                {
                    "phase": phase,
                    "cargo": cargo,
                    "count": count,
                    "distance": distance,
                    "transit_days": transit,
                }
                for phase, cargo, count, distance, transit in TRANSFER_TRACE_CASES
            ],
            "feeder_payment_share_percent": 75,
        },
        "fixture": {
            "path": str(TRANSFER_FIXTURE.relative_to(ROOT)),
            "sha256": sha256_bytes(trace.encode("utf-8")),
            "cases": transfer_trace_case_count(trace),
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


def check_transfer_trace(trace: str, provenance: dict[str, object]) -> None:
    if not TRANSFER_FIXTURE.is_file():
        raise RuntimeError(f"falta fixture versionada: {TRANSFER_FIXTURE}")
    current = TRANSFER_FIXTURE.read_text(encoding="utf-8")
    if current != trace:
        raise RuntimeError(f"DRIFT V1-COAL-TRANSFER: {first_difference(trace, current)}")
    if not TRANSFER_PROVENANCE.is_file():
        raise RuntimeError(f"falta procedencia versionada: {TRANSFER_PROVENANCE}")
    current_provenance = json.loads(TRANSFER_PROVENANCE.read_text(encoding="utf-8"))
    if current_provenance != provenance:
        raise RuntimeError(
            "DRIFT V1-COAL-TRANSFER: la procedencia/hash no coincide con la traza nativa"
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "source",
        nargs="?",
        type=Path,
        default=default_source(),
        help=(
            "checkout OpenTTD fijado (default: reference/openttd-upstream; "
            "override local: OPENTTDRS_PAYMENT_ORACLE_SOURCE)"
        ),
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="comparar contra fixture/procedencia versionadas")
    mode.add_argument("--write", action="store_true", help="actualizar fixture/procedencia desde el oracle nativo")
    args = parser.parse_args(argv)

    try:
        manifest = reference_manifest()
        verify_source(args.source, manifest)
        table = native_table(args.source)
        trace = native_transfer_trace(args.source)
        provenance = expected_provenance(args.source, manifest, table)
        transfer_provenance = expected_transfer_provenance(args.source, manifest, trace)
        if args.check:
            check_fixture(table, provenance)
            check_transfer_trace(trace, transfer_provenance)
            print(f"OK: V1-PAY {provenance['fixture']['cases']} casos nativos; hash {provenance['fixture']['sha256']}")
            print(
                "OK: V1-COAL-TRANSFER "
                f"{transfer_provenance['fixture']['cases']} casos nativos; "
                f"hash {transfer_provenance['fixture']['sha256']}"
            )
        elif args.write:
            FIXTURE.write_text(table, encoding="utf-8")
            PROVENANCE.write_text(json.dumps(provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            TRANSFER_FIXTURE.write_text(trace, encoding="utf-8")
            TRANSFER_PROVENANCE.write_text(
                json.dumps(transfer_provenance, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
            print(
                "Escritos "
                f"{FIXTURE.relative_to(ROOT)}, {PROVENANCE.relative_to(ROOT)}, "
                f"{TRANSFER_FIXTURE.relative_to(ROOT)} y {TRANSFER_PROVENANCE.relative_to(ROOT)}"
            )
        else:
            print(table, end="")
            print(trace, end="")
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"FAIL V1-PAY: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
