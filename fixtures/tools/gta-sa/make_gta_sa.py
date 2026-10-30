#!/usr/bin/env python3
"""Write the invented classic PC GTA:SA save used by the codec tests.

The layout comes from the public GTA:SA save description and the MIT-licensed
010 Editor template by Seemann (x87/GTA-Save-Formats). Every value below is
invented; no bytes of a real save are copied.
"""

from pathlib import Path
import struct

FILE_SIZE = 202752
CHECKSUM_SIZE = 4
BLOCK_10_LEN = 18892

# Marker positions. Block 11 is fixed by block 10's required length; the rest
# are spread out so every body has room and no body contains the word BLOCK.
STARTS = [
    0x0000, 0x0200, 0x0400, 0x0600, 0x0800,
    0x0900, 0x0A00, 0x0B00, 0x0C00, 0x0D00,
    0x1000,
]
STARTS.append(STARTS[10] + 5 + BLOCK_10_LEN)
STARTS.extend(range(0x6000, 0x7500, 0x100))
STARTS.append(0x30000)


def put_u8(buf, at, value):
    buf[at] = value


def put_u32(buf, at, value):
    struct.pack_into("<I", buf, at, value)


def put_i32(buf, at, value):
    struct.pack_into("<i", buf, at, value)


def put_f32(buf, at, value):
    struct.pack_into("<f", buf, at, value)


def data(start):
    return start + 5


def build():
    out = bytearray(FILE_SIZE)
    for start in STARTS:
        out[start : start + 5] = b"BLOCK"

    # Block 0: save name and in-game clock.
    block0 = data(STARTS[0])
    title = b"Invented Save"
    out[block0 + 4 : block0 + 4 + len(title)] = title
    put_u8(out, block0 + 0x86, 9)    # hour
    put_u8(out, block0 + 0x87, 41)   # minute
    put_u8(out, block0 + 0x88, 2)    # day of week

    # Block 2: the first player's health and armour.
    block2 = data(STARTS[2])
    put_u32(out, block2, 1)          # one player
    put_f32(out, block2 + 0x20, 75.5)
    put_f32(out, block2 + 0x24, 12.25)

    # Block 15: money, the screen counter and the maxima.
    block15 = data(STARTS[15])
    put_u32(out, block15, 40)        # block body size written by the game
    put_i32(out, block15 + 0x04, 250000)
    put_i32(out, block15 + 0x10, 250000)
    put_u8(out, block15 + 0x23, 150)
    put_u8(out, block15 + 0x24, 100)

    # The copy of block 15 near the end carries the same values.
    mirror = data(STARTS[30])
    put_u32(out, mirror, 40)
    put_i32(out, mirror + 0x04, 250000)
    put_i32(out, mirror + 0x10, 250000)
    put_u8(out, mirror + 0x23, 150)
    put_u8(out, mirror + 0x24, 100)

    checksum = sum(out) & 0xFFFFFFFF
    struct.pack_into("<I", out, FILE_SIZE - CHECKSUM_SIZE, checksum)
    return bytes(out)


def main():
    root = Path(__file__).resolve().parents[3]
    target = root / "fixtures" / "synthetic" / "gta-sa" / "invented.save"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(build())
    print(target)


if __name__ == "__main__":
    main()
