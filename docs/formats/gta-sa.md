<!-- Behavioral spec written on 2026-10-29 from the public GTA:SA save description and the MIT-licensed 010 Editor template by Seemann (x87/GTA-Save-Formats), with the MIT-licensed libsavegame used to confirm block order and field offsets. Clean room: no game code was decompiled and no third-party editor code or table was copied. -->

# Grand Theft Auto: San Andreas saves

Implemented by `format.gta-sa` (`crates/saveprism-codecs/src/gta_sa/`).

## Sources

- The public GTA:SA save description on GTAMods Wiki, `Saves_(GTA_San_Andreas)`.
- The MIT-licensed 010 Editor template `formats/gtasasave.bt` from
  [x87/GTA-Save-Formats](https://github.com/x87/GTA-Save-Formats), read for behaviour and block
  names.
- The MIT-licensed [libsavegame](https://github.com/gtasa-savegame-editor/libsavegame) by Paul
  Breeuwsma, used to confirm the block order, the `BLOCK` marker count and the field offsets.
- A public 100% complete PC save was used to confirm the offsets: the title `Madd Dogg`, money
  101,456,929, health 128, armour 0, maximum health 128 and maximum armour 100 all read as
  plausible values. That save is not committed; the fixture is invented.

## File layout

Little-endian throughout. The classic PC file is always 202,752 bytes (`0x31800`).

```
0x00000  BLOCK + body                 block 0, misc (clock)
         BLOCK + body                 block 1, scripts
         ...                          blocks 2 to 29
         BLOCK + body                 copy of block 15
         BLOCK + body                 copy of block 16
         BLOCK + body                 copy of block 17
         BLOCK + zero padding         final block
0x317FC  u32 checksum                 sum of every preceding byte
```

A file therefore contains exactly 34 `BLOCK` markers: blocks 0 to 29, the three copies, and the
final padding block. The block bodies are not self-describing. The marker positions are the block
boundaries, and every classic PC save has block 10 at the required 18,892-byte length. The parser
uses that length as the guard against a stray `BLOCK` inside a script body: rather than misread
the file, it refuses it.

### Block 0

The body starts at `marker + 5`:

| Offset | Type | Meaning |
|---|---|---|
| `0x04` | `char[100]` | save name, NUL-terminated |
| `0x86` | `u8` | in-game hour |
| `0x87` | `u8` | in-game minute |
| `0x88` | `u8` | in-game day of week |

`SavePrism` shows the save name read-only. The name is decoded as UTF-8 with replacement
characters; the game writes ASCII in practice.

### Block 2

The body starts with the player count, then the first player record. Only the first player is
shown:

| Offset | Type | Meaning |
|---|---|---|
| `0x00` | `u32` | player count |
| `0x20` | `f32` | health |
| `0x24` | `f32` | armour |

### Block 15

| Offset | Type | Meaning |
|---|---|---|
| `0x00` | `u32` | body size the game wrote (40) |
| `0x04` | `i32` | money |
| `0x10` | `i32` | money shown on screen |
| `0x23` | `u8` | maximum health |
| `0x24` | `u8` | maximum armour |

The copy of block 15 near the end of the file carries the same four values. The writer updates
both copies when one of them is edited.

### Checksum

The last four bytes are a little-endian `u32`: the wrapping sum of every preceding byte. An
unchanged document writes the original bytes exactly, including an already-wrong checksum. When
an edit changes a value, the writer recomputes the sum over the edited bytes.

## What is editable

| Path | Type | Meaning |
|---|---|---|
| `/game_time/hour` | `U8` | in-game hour |
| `/game_time/minute` | `U8` | in-game minute |
| `/game_time/day_of_week` | `U8` | in-game day of week |
| `/player/health` | `F32` | current health |
| `/player/armour` | `F32` | current armour |
| `/player/max_health` | `U8` | maximum health |
| `/player/max_armour` | `U8` | maximum armour |
| `/player/money` | `I32` | money |
| `/player/money_on_screen` | `I32` | money shown on screen |

The health and armour floats must be finite. The maximum values fit a byte, and money is a signed
32-bit value (the game's own editors allow negative money). The title is read-only. The writer
never changes the file length, so block boundaries stay valid.

## Limits and open questions

- Only the classic PC save is handled. The Android/iOS file is 195,002 bytes and the Definitive
  Edition file is 395,926 bytes; both use a different layout and are refused.
- The `BLOCK` marker scan is the same approach used by the MIT `libsavegame`. A save whose script
  body contains the five bytes `BLOCK` before block 11 would fail the block 10 length check and be
  refused.
- Stats, skills (fat, stamina, muscle, sex appeal), weapons, clothes, vehicles and mission state
  are not decoded. They are kept byte-for-byte.
- GTA III and Vice City use their own containers; GTA IV has `format.gta-iv`; GTA V and Red Dead
  Redemption 2 use a RAGE container with an AES-ECB body and a key stored in the game executable.
  None of those are handled here.

## Tests

- `fixtures/synthetic/gta-sa/invented.save`, generated by
  [`fixtures/tools/gta-sa/make_gta_sa.py`](../../fixtures/tools/gta-sa/make_gta_sa.py). The file
  has the canonical 34 markers, a block 10 of exactly 18,892 bytes, and values that exercise the
  block 0, 2, 15 and copied-block-15 paths.
- `crates/saveprism-codecs/src/gta_sa/tests.rs`: detection, exact round trip, every shown field,
  money/health/max-health edits with both block 15 copies and the checksum rewritten, a wrong
  checksum staying wrong when nothing is edited, bad markers and lengths, read-only edits and
  out-of-range integers.
- `fixtures/expected/synthetic/gta-sa/invented.save.toml`: golden detection (`format.gta-sa`,
  exact guard).
- `fuzz/fuzz_targets/gta_sa.rs`: XORs fuzz bytes over the invented file so the header and block
  layout stay reachable; an unchanged decoded file must write back exactly.
