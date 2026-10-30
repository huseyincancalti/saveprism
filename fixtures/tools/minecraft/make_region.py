#!/usr/bin/env python3
"""Generate an invented Minecraft Java region file (`r.0.0.mca`).

The layout follows the public "Region file format" description on minecraft.wiki: an 8 KiB
header of 1024 locations and 1024 timestamps, then 4 KiB sectors. The four invented chunks use
zlib, uncompressed, gzip and an unsupported compression type, so the fixture exercises both
editable chunks and opaque ones. All values are invented.
"""

from pathlib import Path
import gzip
import struct
import zlib

FIXTURES = Path(__file__).resolve().parents[2]
MCA = FIXTURES / "synthetic" / "minecraft-region"
SECTOR = 4096


def tag_byte(value: int) -> bytes:
    return struct.pack(">b", value)


def tag_int(value: int) -> bytes:
    return struct.pack(">i", value)


def tag_long(value: int) -> bytes:
    return struct.pack(">q", value)


def tag_string(value: str) -> bytes:
    raw = value.encode("utf-8")
    return struct.pack(">H", len(raw)) + raw


def compound(fields: list[tuple[str, int, bytes]]) -> bytes:
    out = b""
    for name, kind, payload in fields:
        out += struct.pack(">B", kind) + tag_string(name) + payload
    return out + b"\x00"


def named_root(fields: list[tuple[str, int, bytes]]) -> bytes:
    return b"\x0a" + tag_string("") + compound(fields)


def chunk_nbt(x: int, z: int) -> bytes:
    section = compound(
        [
            ("Y", 1, tag_byte(0)),
            ("block_count", 2, struct.pack(">h", 48)),
        ]
    )
    level = compound(
        [
            ("DataVersion", 3, tag_int(3953)),
            ("xPos", 3, tag_int(x)),
            ("zPos", 3, tag_int(z)),
            ("LevelName", 8, tag_string("Invented")),
            ("Sections", 9, struct.pack(">Bi", 10, 1) + section),
            ("Status", 8, tag_string("full")),
        ]
    )
    return named_root([("Level", 10, level), ("LastUpdate", 4, tag_long(1700000000 + x))])


def payload(compression: int, data: bytes) -> bytes:
    if compression == 1:
        body = gzip.compress(data, mtime=0)
    elif compression == 2:
        body = zlib.compress(data, 6)
    elif compression == 3:
        body = data
    else:
        body = data
    return struct.pack(">IB", len(body) + 1, compression) + body


def main() -> None:
    MCA.mkdir(parents=True, exist_ok=True)
    chunks = [
        (0, 2, chunk_nbt(0, 0)),
        (1, 3, chunk_nbt(1, 0)),
        (2, 1, chunk_nbt(2, 0)),
        (3, 4, b"not an LZ4 chunk"),
    ]
    locations = bytearray(4096)
    timestamps = bytearray(4096)
    body = bytearray()
    sector = 2
    for index, compression, data in chunks:
        encoded = payload(compression, data)
        sectors = (len(encoded) + SECTOR - 1) // SECTOR
        entry = (sector << 8) | sectors
        locations[index * 4 : index * 4 + 4] = entry.to_bytes(4, "big")
        timestamps[index * 4 : index * 4 + 4] = struct.pack(">I", 1700000000 + index)
        body += encoded
        body += b"\x00" * (sectors * SECTOR - len(encoded))
        sector += sectors
    (MCA / "r.0.0.mca").write_bytes(bytes(locations + timestamps + body))


if __name__ == "__main__":
    main()
