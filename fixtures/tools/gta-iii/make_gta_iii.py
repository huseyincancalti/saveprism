#!/usr/bin/env python3
"""Write the invented classic PC GTA III save used by the codec tests.

The layout comes from the public GTA III save description and the MIT-licensed
010 Editor template by Seemann (x87/GTA-Save-Formats). Every value below is
invented; no bytes of a real save are copied.
"""

from pathlib import Path
import struct

FILE_SIZE = 201820
CHECKSUM_SIZE = 4
# Sizes of the 22 top-level blocks in a classic PC save, without their own
# four-byte size fields. Blocks 20 and 21 are padding.
BLOCK_SIZES = [
    25912, 1572, 5488, 12, 24452, 1232, 1036, 9496, 2612, 296, 1548,
    10104, 156, 11552, 7896, 472, 320, 424, 204, 748, 55000, 41196,
]


def put_u8(buf, at, value):
    buf[at] = value


def put_u32(buf, at, value):
    struct.pack_into("<I", buf, at, value)


def build():
    out = bytearray(FILE_SIZE)
    starts = []
    at = 0
    for size in BLOCK_SIZES:
        starts.append(at)
        put_u32(out, at, size)
        at += 4 + size
    assert at == FILE_SIZE - CHECKSUM_SIZE

    # Block 0: save name, island and in-game clock.
    block0 = starts[0] + 4
    title = "Invented Save".encode("utf-16le")
    out[block0 : block0 + len(title)] = title
    put_u32(out, block0 + 0x44, 1)   # Portland
    put_u8(out, block0 + 0x5C, 8)    # in-game hour
    put_u8(out, block0 + 0x60, 27)   # in-game minute

    # Block 1: the first PlayerPed, then the wanted values.
    block1 = starts[1] + 8
    put_u32(out, block1, 1)          # one player
    put_u32(out, block1 + 1534, 4)   # max wanted level
    put_u32(out, block1 + 1538, 1800)  # max chaos

    # Block 16: player money and the on-screen counter.
    block16 = starts[16] + 8
    put_u32(out, block16, 80000)
    put_u32(out, block16 + 0x0F, 80000)

    checksum = sum(out) & 0xFFFFFFFF
    struct.pack_into("<I", out, FILE_SIZE - CHECKSUM_SIZE, checksum)
    return bytes(out)


def main():
    root = Path(__file__).resolve().parents[3]
    target = root / "fixtures" / "synthetic" / "gta-iii" / "invented.save"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(build())
    print(target)


if __name__ == "__main__":
    main()
