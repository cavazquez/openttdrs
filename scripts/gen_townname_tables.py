#!/usr/bin/env python3
"""Genera las tablas Rust del generador de nombres de ciudades de OpenTTD.

Lee ``OpenTTD/src/table/townname.h`` (checkout de referencia) y emite
``openttdrs/crates/openttdrs-core/src/townname/tables.rs`` con las mismas
tablas, byte a byte, para que el puerto Rust de ``townname.cpp`` produzca
exactamente los mismos nombres que OpenTTD.

Uso:
    python3 scripts/gen_townname_tables.py
"""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT.parent / "OpenTTD" / "src" / "table" / "townname.h"
DST = ROOT / "crates" / "openttdrs-core" / "src" / "townname" / "tables.rs"

ALLOW_BITS = {"Short": 1, "Middle": 2, "Long": 4}
CHOOSE_BITS = {"Colour": 1, "Postfix": 2, "NoPostfix": 4}
GENDERS = {
    "CZG_SMASC": "SMasc",
    "CZG_SFEM": "SFem",
    "CZG_SNEUT": "SNeut",
    "CZG_PMASC": "PMasc",
    "CZG_PFEM": "PFem",
    "CZG_PNEUT": "PNeut",
    "CZG_FREE": "Free",
    "CZG_NFREE": "NFree",
}
PATTERNS = {"CZP_JARNI": 0, "CZP_MLADY": 1, "CZP_PRIVL": 2}

STRING_RE = re.compile(r'"((?:[^"\\]|\\.)*)"')


def decode_c_string(raw: str) -> str:
    """Decodifica escapes C (\\uXXXX, \\", \\\\) de un literal."""

    def repl(m: re.Match[str]) -> str:
        esc = m.group(1)
        if esc.startswith("u"):
            return chr(int(esc[1:], 16))
        return {"\\": "\\", '"': '"', "n": "\n", "t": "\t"}[esc]

    return re.sub(r"\\(u[0-9a-fA-F]{4}|.)", repl, raw)


def rust_str(s: str) -> str:
    out = []
    for ch in s:
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif ord(ch) < 0x20 or ord(ch) > 0x7E:
            out.append(f"\\u{{{ord(ch):04x}}}")
        else:
            out.append(ch)
    return '"' + "".join(out) + '"'


def array_body(text: str, name: str, suffix: str = r"\[\]") -> str:
    m = re.search(
        rf"static const [\w:]+ {name}{suffix} = \{{(.*?)\n\}};", text, re.DOTALL
    )
    if m is None:
        raise SystemExit(f"no se encontró la tabla {name}")
    return m.group(1)


def parse_string_array(text: str, name: str) -> list[str]:
    return [decode_c_string(s) for s in STRING_RE.findall(array_body(text, name))]


def flags_value(token: str, bits: dict[str, int], all_const: str) -> int:
    token = token.strip()
    if token in (all_const,):
        return sum(bits.values())
    if token == "{}" or token == "":
        return 0
    value = 0
    for part in re.findall(r"(?:CzechAllowFlag|CzechChooseFlag)::(\w+)", token):
        value |= bits[part]
    return value


def split_row(row: str) -> list[str]:
    """Separa los campos de una fila C a nivel superior (respeta `{...}`)."""
    fields, depth, cur = [], 0, ""
    for ch in row:
        if ch == "," and depth == 0:
            fields.append(cur.strip())
            cur = ""
            continue
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        cur += ch
    if cur.strip():
        fields.append(cur.strip())
    return fields


ROW_RE = re.compile(r"\{((?:[^{}]|\{[^{}]*\})*)\}")


def parse_rows(text: str, name: str) -> list[list[str]]:
    return [split_row(m.group(1)) for m in ROW_RE.finditer(array_body(text, name))]


def emit_string_array(out: list[str], rust_name: str, values: list[str]) -> None:
    out.append(f"pub(super) static {rust_name}: &[&str] = &[")
    for v in values:
        out.append(f"    {rust_str(v)},")
    out.append("];\n")


def main() -> None:
    text = SRC.read_text(encoding="utf-8")

    out: list[str] = [
        "//! Tablas del generador de nombres de ciudades de `OpenTTD`.",
        "//!",
        "//! Generado por `scripts/gen_townname_tables.py` desde",
        "//! `OpenTTD/src/table/townname.h`. NO EDITAR A MANO.",
        "",
        "use super::{CzechGender, CzechNameAdj, CzechNameSubst};",
        "",
    ]

    simple_names = re.findall(
        r"static const std::string_view (_name_\w+)\[\] =", text
    )
    for c_name in simple_names:
        rust_name = c_name.lstrip("_").upper()
        emit_string_array(out, rust_name, parse_string_array(text, c_name))

    # _name_czech_patmod[][3]: sufijos de adjetivos por [género][patrón].
    body = array_body(text, "_name_czech_patmod", suffix=r"\[\]\[3\]")
    patmod_rows = [STRING_RE.findall(m.group(1)) for m in ROW_RE.finditer(body)]
    out.append("pub(super) static NAME_CZECH_PATMOD: [[&str; 3]; 6] = [")
    for row in patmod_rows:
        cells = ", ".join(rust_str(decode_c_string(s)) for s in row)
        out.append(f"    [{cells}],")
    out.append("];\n")

    # CzechNameAdj { pattern, choose, name }
    out.append("pub(super) static NAME_CZECH_ADJ: &[CzechNameAdj] = &[")
    for pattern, choose, name in parse_rows(text, "_name_czech_adj"):
        p = PATTERNS[pattern]
        c = flags_value(choose, CHOOSE_BITS, "CZC_ANY")
        n = rust_str(decode_c_string(STRING_RE.search(name).group(1)))
        out.append(f"    CzechNameAdj {{ pattern: {p}, choose: {c}, name: {n} }},")
    out.append("];\n")

    # CzechNameSubst { gender, allow, choose, name }
    for c_name in ("_name_czech_subst_full", "_name_czech_subst_stem", "_name_czech_subst_ending"):
        rust_name = c_name.lstrip("_").upper()
        out.append(f"pub(super) static {rust_name}: &[CzechNameSubst] = &[")
        for gender, allow, choose, name in parse_rows(text, c_name):
            g = GENDERS[gender.strip()]
            a = flags_value(allow, ALLOW_BITS, "CZA_ALL")
            c = flags_value(choose, CHOOSE_BITS, "CZC_ANY")
            n = rust_str(decode_c_string(STRING_RE.search(name).group(1)))
            out.append(
                f"    CzechNameSubst {{ gender: CzechGender::{g}, allow: {a}, choose: {c}, name: {n} }},"
            )
        out.append("];\n")

    DST.parent.mkdir(parents=True, exist_ok=True)
    DST.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"OK: {DST} ({len(simple_names)} tablas de strings + checas)")


if __name__ == "__main__":
    main()
