<!-- Behavioral spec written on 2026-10-29 from the public GameMaker manual and the open-source GameMaker HTML5 runtime's collection serialization. Clean room: no runtime source was copied. -->

# GameMaker `ds_map` and `ds_list` dumps

Implemented by `format.gamemaker-ds-map` and `format.gamemaker-ds-list`
(`crates/saveprism-codecs/src/gamemaker/`).

## Sources

- The GameMaker manual pages for `ds_map_write`, `ds_map_read`, `ds_list_write` and
  `ds_list_read`. They state that the text format changed after Studio 1.2 and again in 2.3.1 and
  that arrays are stored too.
- The public reverse-engineered `ds_map` note at
  [`kikones34/dsmap-serial-format`](https://github.com/kikones34/dsmap-serial-format) for the
  original magic 402 layout. Only the byte layout was read.
- The open-source [GameMaker HTML5 runtime](https://github.com/YoYoGames/GameMaker-HTML5),
  `scripts/yyVariable.js` (`variable_WriteValue` / `variable_ReadValue`) and
  `scripts/functions/collections/ds_map.js`, `ds_list.js`, read for the type codes, the version 3
  array layout and the current magic numbers. No runtime code was copied.

Invented fixtures (`fixtures/synthetic/gamemaker/ds-map*.txt`, `ds-list*.txt`) cover magic 402 and
403 maps, magic 303 and 302 lists, every scalar type, 1D arrays and the legacy 2D array layout. The
generator is `fixtures/tools/gamemaker/make_gamemaker.py`.

## Text and byte layout

The text is two hexadecimal digits per byte, upper or lower case, with optional trailing ASCII
whitespace. The bytes are little-endian.

```
u32 magic                 402 / 403 for maps, 302 / 303 for lists
u32 count                 number of map entries or list items
entry or item ...         count times
```

Magic 402 and 302 use the version 3 value layout; 403 and 303 use the current layout. The
difference is only in arrays.

### Values

| Type | Code | Payload |
|---|---|---|
| real | 0 | IEEE-754 `f64` |
| string | 1 | `u32` byte length, then that many bytes |
| array | 2 | see below |
| pointer | 3 | `i64` |
| undefined | 5 | none |
| int32 | 7 | `i32` |
| int64 | 10 | `i64` |
| bool | 13 | `f64` (zero is false) |

In the current layout an array is `u32 length`, then that many values. In the version 3 layout an
array starts with `u32 1` and a length, which is how a 1D array is written; a real 2D array starts
with its row count (not 1), and each row carries its own length and values. An array with one row
is therefore read as a 1D array, exactly as the runtime reads it.

A map key can be any scalar value but not an array. Keys are read-only.

## Tree

A map root is a read-only `Map` (`GM:map`). A list root is a read-only `Array` (`GM:list`).

| Value | Node | Hint | Editable |
|---|---|---|---|
| real | `Float` (F64) | `GM:real`, keys `GM:key:real` | yes |
| string | `Str`, or read-only `Bytes` when not UTF-8 | `GM:string` | text yes |
| array | `Array` | `GM:array` | items |
| 2D array | `Array` of row arrays | `GM:array2d` | items |
| pointer | `Int` (I64) | `GM:ptr` | yes |
| undefined | `Null` | `GM:undefined` | no |
| int32 | `Int` (I32) | `GM:int32` | yes |
| int64 | `Int` (I64) | `GM:int64` | yes |
| bool | `Bool` | `GM:bool` | yes |

Map keys keep their original type, so a numeric key stays numeric. Nested array containers are
read-only, but their items are editable.

## Writing

The original text is the codec parameter `original`, the trailing whitespace is `trailing`, and
the magic number is `magic`. An unchanged dump returns the original bytes, including lowercase
hex. An edit writes the canonical uppercase dump with the same magic, entry order, types and
array layout. String lengths and counts are recomputed. Unknown magics and value codes are
refused, not guessed.

Limits: the document node limit applies to entries and nested arrays; input and node limits are
checked before allocation.
