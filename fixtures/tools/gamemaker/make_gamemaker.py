#!/usr/bin/env python3
"""Generate invented GameMaker and Pixel Game Maker MV save fixtures.

GameMaker saves are commonly an INI file (`ini_open`), a `json_encode` / `json_stringify`
document, or the hexadecimal dump of `ds_map_write` / `ds_list_write`. The dumps follow the
public layout of the open-source HTML5 runtime: little-endian magic and count, then typed values
(`variable_WriteValue`). Maps use 402 or 403, lists 302 or 303. The Pixel Game Maker MV save is
the plain JSON file the engine keeps at `%LOCALAPPDATA%/<project>/save/play_LOADFILE#.json`.
All values are invented.
"""

from pathlib import Path
import base64
import json
import struct

FIXTURES = Path(__file__).resolve().parents[2]
GM = FIXTURES / "synthetic" / "gamemaker"
PGM = FIXTURES / "synthetic" / "pixel-game-maker-mv"

PLAYER = {
    "player": {"name": "Mira", "gold": 120, "level": 7},
    "items": ["sword", "shield"],
}


def real(value: float) -> bytes:
    return struct.pack("<I", 0) + struct.pack("<d", value)


def text(value: str) -> bytes:
    raw = value.encode("utf-8")
    return struct.pack("<II", 1, len(raw)) + raw


def int32(value: int) -> bytes:
    return struct.pack("<Ii", 7, value)


def int64(value: int) -> bytes:
    return struct.pack("<Iq", 10, value)


def boolean(value: bool) -> bytes:
    return struct.pack("<Id", 13, 1.0 if value else 0.0)


def undefined() -> bytes:
    return struct.pack("<I", 5)


def array(values: list[bytes], version: int) -> bytes:
    if version == 3:
        return struct.pack("<III", 2, 1, len(values)) + b"".join(values)
    return struct.pack("<II", 2, len(values)) + b"".join(values)


def array_2d(rows: list[list[bytes]]) -> bytes:
    out = struct.pack("<II", 2, len(rows))
    for row in rows:
        out += struct.pack("<I", len(row)) + b"".join(row)
    return out


def ds_map() -> bytes:
    entries = [
        (text("gold"), real(120.0)),
        (text("name"), text("Mira")),
        (real(3.14), text("pi")),
    ]
    out = struct.pack("<II", 402, len(entries))
    for key, value in entries:
        out += key + value
    return out


def ds_map_v403() -> bytes:
    entries = [
        (text("gold"), real(120.0)),
        (text("level"), int32(7)),
        (text("seed"), int64(-9007199254740993)),
        (text("alive"), boolean(True)),
        (text("note"), undefined()),
        (text("items"), array([text("sword"), text("shield")], 0)),
        (text("switches"), array([boolean(True), boolean(False)], 0)),
    ]
    out = struct.pack("<II", 403, len(entries))
    for key, value in entries:
        out += key + value
    return out


def ds_list() -> bytes:
    values = [
        real(3.5),
        text("Mira"),
        int32(-12),
        int64(9007199254740993),
        boolean(False),
        undefined(),
        array([real(1.0), text("two")], 0),
    ]
    return struct.pack("<II", 303, len(values)) + b"".join(values)


def ds_list_legacy() -> bytes:
    values = [
        real(1.5),
        array([real(1.0), text("two")], 3),
        array_2d([[real(1.0), real(2.0)], [text("three")]]),
    ]
    return struct.pack("<II", 302, len(values)) + b"".join(values)


def main() -> None:
    GM.mkdir(parents=True, exist_ok=True)
    PGM.mkdir(parents=True, exist_ok=True)

    (GM / "save.ini").write_bytes(
        b'[player]\nname=Mira\ngold=120\ntitle="The Hero"\n\n[flags]\nmet_guard="1"\n',
    )
    (GM / "save.json").write_bytes(
        json.dumps(PLAYER, separators=(",", ":")).encode("utf-8"),
    )
    (GM / "save-base64.txt").write_bytes(
        base64.b64encode(json.dumps(PLAYER, separators=(",", ":")).encode("utf-8")),
    )
    dump = ds_map().hex().upper()
    (GM / "ds-map.txt").write_bytes(dump.encode("ascii") + b"\n")
    (GM / "ds-map-lower.txt").write_bytes(dump.lower().encode("ascii"))
    (GM / "ds-map-v403.txt").write_bytes(ds_map_v403().hex().upper().encode("ascii") + b"\n")
    (GM / "ds-list.txt").write_bytes(ds_list().hex().upper().encode("ascii") + b"\n")
    (GM / "ds-list-legacy.txt").write_bytes(
        ds_list_legacy().hex().upper().encode("ascii") + b"\n"
    )

    (PGM / "play_LOADFILE1.json").write_bytes(
        json.dumps(
            {
                "variables": {"gold": 120, "hp": 35, "name": "Mira"},
                "switches": {"met_guard": True},
                "version": 1,
            },
            indent=2,
        ).encode("utf-8")
        + b"\n",
    )


if __name__ == "__main__":
    main()
