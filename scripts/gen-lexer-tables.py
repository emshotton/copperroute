#!/usr/bin/env python3
"""Decode the packed JFlex DFA tables out of `SpecctraDsnStreamReader.java`.

One-shot, committed, re-runnable generator (Plan 3 ruling 3): the Specctra lexer's DFA is
ported by transcribing JFlex's packed tables mechanically, never by hand-writing a recognizer.
It reads the Java scanner source, extracts the five `ZZ_*_PACKED*` string literals, runs
JFlex 1.4.1's four unpack algorithms verbatim (the Java bodies live at
SpecctraDsnStreamReader.java:616-722) and writes `crates/copper-dsn/src/lexer/tables.rs`.

Usage (from the repository root):

    python3 scripts/gen-lexer-tables.py \
        [--java ../freerouting/src/main/java/app/freerouting/io/specctra/parser/SpecctraDsnStreamReader.java] \
        [--out crates/copper-dsn/src/lexer/tables.rs]

The generated file is committed too: a build script would need the sibling `../freerouting`
checkout at build time, which CI cannot assume.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_JAVA = (
    ROOT.parent
    / "freerouting/src/main/java/app/freerouting/io/specctra/parser/SpecctraDsnStreamReader.java"
)
DEFAULT_OUT = ROOT / "crates/copper-dsn/src/lexer/tables.rs"


def java_string_literal(text: str) -> list[int]:
    """Decode a Java string literal body into its UTF-16 code units.

    Handles the escapes JFlex emits: octal (`\\11`, `\\136`, one to three digits) and
    `\\uXXXX`. Any other backslash escape is a signal that this scanner is not the one this
    generator was written for, so it is a hard error rather than a silent guess.
    """
    units: list[int] = []
    i = 0
    n = len(text)
    while i < n:
        ch = text[i]
        if ch != "\\":
            code = ord(ch)
            if code > 0xFFFF:  # would be a surrogate pair; JFlex never emits one
                raise ValueError(f"non-BMP character in packed literal at {i}")
            units.append(code)
            i += 1
            continue
        i += 1
        esc = text[i]
        if esc == "u":
            i += 1
            while text[i] == "u":  # Java allows \uuuu1234
                i += 1
            units.append(int(text[i : i + 4], 16))
            i += 4
        elif esc in "01234567":
            digits = ""
            # Java octal escapes are 1-3 digits, and 3 only when the first is 0-3.
            while len(digits) < 3 and i < n and text[i] in "01234567":
                if len(digits) == 2 and digits[0] not in "0123":
                    break
                digits += text[i]
                i += 1
            units.append(int(digits, 8))
        else:
            raise ValueError(f"unsupported Java escape \\{esc} at {i}")
    return units


def extract_packed(source: str, name: str) -> list[int]:
    """Concatenate the `+`-continued string literals of `private static final String <name>`."""
    m = re.search(
        r"private static final String " + re.escape(name) + r"\s*=\s*(.*?);",
        source,
        re.DOTALL,
    )
    if m is None:
        raise SystemExit(f"error: {name} not found in the Java source")
    body = m.group(1)
    parts = re.findall(r'"((?:[^"\\]|\\.)*)"', body)
    if not parts:
        raise SystemExit(f"error: {name} has no string literal parts")
    units: list[int] = []
    for part in parts:
        units.extend(java_string_literal(part))
    return units


# --- JFlex 1.4.1 unpack algorithms, transcribed from the Java ------------------------------


def unpack_cmap(packed: list[int], limit: int) -> list[int]:
    """`zzUnpackCMap` (SpecctraDsnStreamReader.java:704) — run-length (count, value) pairs.

    The Java loop is bounded by the literal packed length (`while (i < 274)`), which this
    generator asserts against the literal it actually decoded. `[value] * count` differs from
    Java's `do { … } while (--count > 0)` only for `count == 0`, which would emit one entry
    there and none here — and the unpacked-length assertions below would catch that.
    """
    if len(packed) != limit:
        raise SystemExit(
            f"error: ZZ_CMAP_PACKED is {len(packed)} code units, but zzUnpackCMap stops at {limit}"
        )
    out: list[int] = []
    i = 0
    while i < limit:
        count = packed[i]
        i += 1
        value = packed[i]
        i += 1
        out.extend([value] * count)
    return out


def unpack_pairs(packed: list[int], size: int, decrement: bool) -> list[int]:
    """`zzUnpackAction`/`zzUnpackAttribute` (`decrement=False`) and `zzUnpackTrans` (`True`).

    As in `unpack_cmap`, `[value] * count` differs from Java's `do`-`while` only for `count == 0`,
    which the length check at the end of this function would catch.
    """
    out: list[int] = []
    i = 0
    while i < len(packed):
        count = packed[i]
        i += 1
        value = packed[i]
        i += 1
        if decrement:
            value -= 1
        out.extend([value] * count)
    if len(out) != size:
        raise SystemExit(f"error: unpacked {len(out)} entries, Java allocates {size}")
    return out


def unpack_rowmap(packed: list[int], size: int) -> list[int]:
    """`zzUnpackRowMap` (SpecctraDsnStreamReader.java:644) — `(high << 16) | low`."""
    out: list[int] = []
    i = 0
    while i < len(packed):
        high = packed[i] << 16
        i += 1
        out.append(high | packed[i])
        i += 1
    if len(out) != size:
        raise SystemExit(f"error: unpacked {len(out)} row map entries, Java allocates {size}")
    return out


# --- emission ------------------------------------------------------------------------------


def java_int(source: str, pattern: str, base: int = 10) -> int:
    m = re.search(pattern, source)
    if m is None:
        raise SystemExit(f"error: {pattern} not found in the Java source")
    return int(m.group(1), base)


def render(name: str, doc: str, ty: str, values: list[int], per_line: int) -> str:
    lines = [f"/// {doc}", f"pub static {name}: [{ty}; {len(values)}] = ["]
    for start in range(0, len(values), per_line):
        chunk = values[start : start + per_line]
        lines.append("    " + " ".join(f"{v}," for v in chunk))
    lines.append("];")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--java", type=Path, default=DEFAULT_JAVA)
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT)
    args = ap.parse_args()

    java_path: Path = args.java
    source = java_path.read_text(encoding="utf-8")

    blob = subprocess.run(
        ["git", "hash-object", str(java_path)], capture_output=True, text=True, check=True
    ).stdout.strip()

    cmap_limit = java_int(source, r"while \(i < (\d+)\) \{\n      int count = packed\.charAt")
    cmap_size = java_int(source, r"char\[\] map = new char\[0x([0-9A-Fa-f]+)\];", base=16)
    action_size = java_int(
        source, r"zzUnpackAction\(\) \{\n    int\[\] result = new int\[(\d+)\]"
    )
    rowmap_size = java_int(
        source, r"zzUnpackRowMap\(\) \{\n    int\[\] result = new int\[(\d+)\]"
    )
    trans_size = java_int(source, r"zzUnpackTrans\(\) \{\n    int\[\] result = new int\[(\d+)\]")
    attr_size = java_int(
        source, r"zzUnpackAttribute\(\) \{\n    int\[\] result = new int\[(\d+)\]"
    )
    buffer_size = java_int(source, r"ZZ_BUFFERSIZE = (\d+) \* 1024 \* 1024") * 1024 * 1024

    cmap = unpack_cmap(extract_packed(source, "ZZ_CMAP_PACKED"), cmap_limit)
    if len(cmap) != cmap_size:
        raise SystemExit(f"error: ZZ_CMAP unpacked to {len(cmap)}, Java allocates {cmap_size}")
    action = unpack_pairs(extract_packed(source, "ZZ_ACTION_PACKED_0"), action_size, False)
    rowmap = unpack_rowmap(extract_packed(source, "ZZ_ROWMAP_PACKED_0"), rowmap_size)
    trans = unpack_pairs(extract_packed(source, "ZZ_TRANS_PACKED_0"), trans_size, True)
    attribute = unpack_pairs(extract_packed(source, "ZZ_ATTRIBUTE_PACKED_0"), attr_size, False)

    # The scanner has no ZZ_LEXSTATE table: JFlex omits it when the specification uses no
    # `%bol`/BOL-sensitive rules, and the driver seeds `zzState = zzLexicalState` directly
    # (SpecctraDsnStreamReader.java:899). Nothing to emit.

    print(f"ZZ_CMAP:      {len(cmap)} entries, max class {max(cmap)}", file=sys.stderr)
    print(f"ZZ_ACTION:    {len(action)} entries, max {max(action)}", file=sys.stderr)
    print(f"ZZ_ROWMAP:    {len(rowmap)} entries, max {max(rowmap)}", file=sys.stderr)
    print(f"ZZ_TRANS:     {len(trans)} entries, min {min(trans)} max {max(trans)}", file=sys.stderr)
    print(f"ZZ_ATTRIBUTE: {len(attribute)} entries, max {max(attribute)}", file=sys.stderr)

    header = f"""//! GENERATED by scripts/gen-lexer-tables.py — do not edit.
//!
//! JFlex 1.4.1 DFA tables for the Specctra DSN scanner, unpacked from the five
//! `ZZ_*_PACKED*` string literals of
//! `io/specctra/parser/SpecctraDsnStreamReader.java` (git blob {blob})
//! by JFlex's own `zzUnpackCMap`/`zzUnpackAction`/`zzUnpackRowMap`/`zzUnpackTrans`/
//! `zzUnpackAttribute` (that file's lines 616-722). Regenerate with:
//!
//! ```text
//! python3 scripts/gen-lexer-tables.py
//! ```
//!
//! `ZZ_CMAP` is indexed by a **UTF-16 code unit**: Java's `zzUnpackCMap` builds a
//! `char[0x10000]`, so the table's domain is exactly `0..=0xFFFF` and the scanner's buffer
//! holds `u16` code units rather than Rust `char`s (see `super::DsnScanner::buffer`).
//!
//! This scanner has **no `ZZ_LEXSTATE` table** — JFlex emits one only for BOL-sensitive
//! specifications; here the driver seeds `zz_state` from the lexical state directly
//! (`SpecctraDsnStreamReader.java:899`).

/// `ZZ_BUFFERSIZE` (`SpecctraDsnStreamReader.java:40`): the fixed size of Java's `zzBuffer`.
pub const ZZ_BUFFERSIZE: usize = {buffer_size};
"""

    body = "\n\n".join(
        [
            render(
                "ZZ_CMAP",
                "Translates characters to character classes (`ZZ_CMAP`).",
                "u16",
                cmap,
                32,
            ),
            render(
                "ZZ_ACTION",
                "Translates DFA states to action switch labels (`ZZ_ACTION`).",
                "i32",
                action,
                20,
            ),
            render(
                "ZZ_ROWMAP",
                "Translates a state to a row index in the transition table (`ZZ_ROWMAP`).",
                "i32",
                rowmap,
                20,
            ),
            render("ZZ_TRANS", "The transition table of the DFA (`ZZ_TRANS`).", "i32", trans, 20),
            render(
                "ZZ_ATTRIBUTE",
                "`ZZ_ATTRIBUTE[state]` contains the attributes of state `state`.",
                "i32",
                attribute,
                20,
            ),
        ]
    )

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(header + "\n" + body + "\n", encoding="utf-8")
    # Run rustfmt so that re-running this generator is idempotent against a `cargo fmt` tree.
    subprocess.run(["rustfmt", "--edition", "2024", str(args.out)], check=True)
    print(f"wrote {args.out} ({args.out.stat().st_size} bytes)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
