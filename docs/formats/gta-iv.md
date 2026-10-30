<!-- Behavioral spec written on 2026-10-29 from the public GTA IV save description and the MIT-licensed 010 Editor template by Seemann (x87/GTA-Save-Formats). Clean room: no game code was decompiled and no third-party editor code or table was copied. -->

# Grand Theft Auto IV saves

Implemented by `format.gta-iv` (`crates/saveprism-codecs/src/gta4/`).

## Sources

- The public GTA IV save description on the GTAMods Wiki, `Saves_(GTA_4)`.
- The MIT-licensed 010 Editor template `formats/gta4save.bt` from
  [x87/GTA-Save-Formats](https://github.com/x87/GTA-Save-Formats), read for behaviour and block
  names. No template code was copied.
- A public 100% complete PC save was used to check the `PlayerInfo` offsets: money 665356, maximum
  health 200, maximum armour 100, health 200 and armour 100 all read as plausible values at the
  offsets below. That save is not committed; the fixture is invented.

The file is a PC save (`SGTA4xx`). Episodes from Liberty City use the same container; the last
mission name is prefixed with `TLAD - ` or `TBoGT - `.

## File layout

Little-endian throughout.

```
0x000  metadata                                 0x110 bytes
0x110  BLOCK + u32 size + data                  block 0, SimpleVars
       BLOCK + u32 size + data                  block 1, PlayerInfo
       ...                                      blocks 2 to 31
       u32 checksum
       END\0 + 0x168 bytes                      end block, 0x16C bytes
```

### Metadata

| Offset | Type | Meaning |
|---|---|---|
| `0x00` | `u32` | save version (`0x39` in the checked save; the value comes from `common/data/version.txt`) |
| `0x04` | `u32` | savegame size in bytes |
| `0x08` | `u32` | global variables size |
| `0x0C` | `char[4]` | `SAVE` |
| `0x10` | `wchar_t[128]` | last mission passed, NUL-terminated UTF-16LE |

`SavePrism` shows the version, file size, globals size and last mission read-only. The metadata is
never rewritten, so the file size field stays as the game wrote it.

### Data blocks

Each block starts with `BLOCK` and a `u32` size. The size includes the nine header bytes, so the
next block starts at `start + size`. An empty block is nine bytes. The game writes 32 blocks in
this order:

| # | Name | # | Name |
|---|---|---|---|
| 0 | SimpleVars | 16 | Radio |
| 1 | PlayerInfo | 17 | Objects |
| 2 | ExtraContent | 18 | Relationships |
| 3 | Scripts | 19 | Inventory |
| 4 | Garages | 20 | Pools |
| 5 | GameLogic | 21 | PhoneInfo |
| 6 | PathFind | 22 | AudioScriptObject |
| 7 | Pickups | 23 | SetPieces |
| 8 | Restart | 24 | Streaming |
| 9 | Radar | 25 | PedType |
| 10 | Zones | 26 | Tags |
| 11 | Gangs | 27 | Shopping |
| 12 | CarGenerators | 28 | GangWars |
| 13 | Stats | 29 | EntryExits |
| 14 | IplStore | 30 | 3dMarkers |
| 15 | StuntJumps | 31 | Vehicles |

`SavePrism` reads two blocks:

- `SimpleVars` (block 0), data offset `block_start + 9`:
  - `0x28` `u32` month
  - `0x2C` `u32` day
  - `0x30` `u32` hour
  - `0x34` `u32` minute
  - `0x38` `u32` day of week
  - `0x80` `u32` maximum wanted level
  - `0x84` `u32` maximum chaos level
- `PlayerInfo` (block 1), data offset `block_start + 9`:
  - `0x18` `u32` player money
  - `0x20` `u32` money shown on screen
  - `0x34` `u16` maximum health
  - `0x36` `u16` maximum armour
  - `0x60` `f32` health
  - `0x64` `f32` armour

Every other block, including the stats, missions, pickups and vehicles, is left byte-identical
and is not shown. The block structure is still checked on every read: a missing `BLOCK`, a size
below nine, a block past the end of the file, a missing `END\0` or a cut-short end block is a
parse error.

### Checksum

The `u32` after block 31 is the wrapping sum of every preceding byte. The game ignores it, so an
unchanged document writes the original bytes exactly, even if the checksum was already wrong.
When an edit changes a value, the writer recomputes the sum over the edited bytes.

GTA IV adds the Games For Windows Live data size after calculating the checksum, which is why the
metadata size can be 12 bytes larger than the sum's range. That difference does not matter here:
the writer never changes the metadata size, and the game does not verify the checksum.

### End block

The end block is always `0x16C` bytes and starts with `END\0`. Its remaining bytes are Games For
Windows Live data; SavePrism keeps them unchanged.

## What is editable

| Path | Type | Meaning |
|---|---|---|
| `/game_time/month` | `U32` | in-game month |
| `/game_time/day` | `U32` | in-game day |
| `/game_time/hour` | `U32` | in-game hour |
| `/game_time/minute` | `U32` | in-game minute |
| `/game_time/day_of_week` | `U32` | in-game day of week |
| `/player/money` | `U32` | player money |
| `/player/money_on_screen` | `U32` | money shown on screen |
| `/player/max_health` | `U16` | maximum health |
| `/player/max_armour` | `U16` | maximum armour |
| `/player/health` | `F32` | current health |
| `/player/armour` | `F32` | current armour |
| `/max_wanted_level` | `U32` | maximum wanted level |
| `/max_chaos` | `U32` | maximum chaos level |

The health float must be finite and must fit an `f32` after conversion; the integers must fit
their node width. The metadata and last mission are read-only. The writer never changes the file
length, so editing a field cannot corrupt the block sizes.

## Limits and open questions

- No encryption and no compression.
- The stats block is not decoded. Its offset of the first stat is `0x54` in older versions and
  `0x6C` in newer ones, and the stat types vary by id, so exposing individual stats needs a table
  that is not in the public description used here.
- The `Scripts`, `Garages`, `GameLogic` and similar blocks are opaque. They are kept byte-for-byte.
- GTA III, Vice City and San Andreas use a different container (San Andreas has a fixed-size file
  with `BLOCK` records); they are not handled by this codec.
- GTA V and Red Dead Redemption 2 use a RAGE container with an AES-ECB body and a key stored in the
  game executable. They are not GTA IV saves and are not handled here.

## Tests

- `fixtures/synthetic/gta-iv/invented.save`, generated by
  [`fixtures/tools/gta-iv/make_gta_iv.py`](../../fixtures/tools/gta-iv/make_gta_iv.py).
- `crates/saveprism-codecs/src/gta4/tests.rs`: detection, exact round trip, every shown field,
  edits of money/health/max health, checksum recomputation, a wrong checksum staying wrong when
  nothing is edited, bad block and end markers, read-only edits and out-of-range integers.
- `fixtures/expected/synthetic/gta-iv/invented.save.toml`: golden detection (`format.gta-iv`,
  exact guard).
- `fuzz/fuzz_targets/gta4.rs`: keeps a fixed metadata and `SimpleVars` prefix, then fuzzes the
  remaining blocks and end block; an unchanged decoded file must write back exactly.
