#!/usr/bin/env python3
"""Write the invented GTA IV save used by the codec tests.

The layout comes from the public GTA IV save description and the MIT-licensed
010 Editor template by Seemann (x87/GTA-Save-Formats).  Every value below is
invented; no bytes of a real save are copied.
"""

from pathlib import Path
import struct

META_SIZE = 0x110
CHECKSUM_SIZE = 4
END_SIZE = 0x16C


def put_u32(buf, at, value):
    struct.pack_into("<I", buf, at, value)


def put_u16(buf, at, value):
    struct.pack_into("<H", buf, at, value)


def put_f32(buf, at, value):
    struct.pack_into("<f", buf, at, value)


def block(size, fill=b"\0"):
    out = bytearray(size)
    out[:5] = b"BLOCK"
    put_u32(out, 5, size)
    out[9:] = fill * (size - 9)
    return out


def build():
    meta = bytearray(META_SIZE)
    put_u32(meta, 0x00, 0x39)  # version 1.0.7.0 / 1.0.8.0 style
    put_u32(meta, 0x08, 0x1FE)  # global variables size
    meta[0x0C:0x10] = b"SAVE"
    title = "Invented Mission".encode("utf-16le")
    meta[0x10 : 0x10 + len(title)] = title

    simple = block(0xB9)
    put_u32(simple, 9 + 0x28, 5)  # month
    put_u32(simple, 9 + 0x2C, 14)  # day
    put_u32(simple, 9 + 0x30, 17)  # hour
    put_u32(simple, 9 + 0x34, 21)  # minute
    put_u32(simple, 9 + 0x38, 3)  # day of week
    put_u32(simple, 9 + 0x80, 6)  # max wanted level
    put_u32(simple, 9 + 0x84, 6900)  # max chaos

    player = block(0xD9)
    put_u32(player, 9 + 0x18, 123456)  # money
    put_u32(player, 9 + 0x20, 123456)  # money on screen
    put_u16(player, 9 + 0x34, 200)  # max health
    put_u16(player, 9 + 0x36, 100)  # max armour
    put_f32(player, 9 + 0x60, 150.5)  # health
    put_f32(player, 9 + 0x64, 25.25)  # armour

    blocks = [simple, player] + [block(9) for _ in range(30)]
    body = b"".join(blocks)
    total = META_SIZE + len(body) + CHECKSUM_SIZE + END_SIZE
    put_u32(meta, 0x04, total)

    end = bytearray(END_SIZE)
    end[0:4] = b"END\0"
    put_u32(end, 4, 0x128)

    out = bytearray(meta + body)
    checksum = sum(out) & 0xFFFFFFFF
    out += struct.pack("<I", checksum)
    out += end
    return bytes(out)


def main():
    root = Path(__file__).resolve().parents[3]
    target = root / "fixtures" / "synthetic" / "gta-iv" / "invented.save"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(build())
    print(target)


if __name__ == "__main__":
    main()
