# F75 Control Center — mechanics

Everything below was recovered from the installer payload and the vendor driver.
The installer itself (`AULA F75 driver.exe`) is an Inno Setup 5.3.3 wrapper. The
real program is `app/OemDrv.exe` (32-bit MFC, 3347 functions) and the keyboard
profile is `app/Dev/kb/F75KR/KB.ini`.

The app in this folder is an independent Linux reimplementation. It detects the
keyboard through `/sys/class/hidraw` and writes lighting, key remaps and macro
assignments over the 1032-byte HID feature report. The music page stays local
because its real-time stream is a different protocol (see “Not implemented”).

## Recovered driver functions

All addresses are in the extracted `OemDrv.exe` image (Ghidra 12.1.4, Deep mode).
They are the source for the values below.

| Address | Function | What it does |
|---|---|---|
| `0x410A70` | F75 `KB.ini` parser | Reads `SleepUI/SleepHW`, `Light`, `Speed`, `LedOpt1..24`, `MatrixLen`, `RGBIndex`, `GameRGBIndex`, `DefLedIndex`, `Gaoshou*`, macro limits and colour defaults into `devState` |
| `0x40F330` | generic/older config loader | Supplies the same defaults for a different device class; parses `DebounceUI/HW` but discards the lists |
| `0x489080` | `CDev916KB::ApplySetting` | Sends macros, key matrix, config/profile and the LED colour matrix, with the delays below |
| `0x487DA0` | LED feature-report builder | Builds `09 <cmd> 00 40 00 00 00` + 1024-byte payload |
| `0x4879F0` | low-level feature sender | Prepends the report id and calls `HidD_SetFeature` up to three times, 200 ms apart |
| `0x490680` | 520-byte config frame | Builds and sends the `0x208`-byte frame, checks the reply and retries |
| `0x4881F0` | macro sender | Called with macro ids `0xDC/0xE0/0xE4/0xE8` and `0x400`-byte buffers (body not decoded) |
| `0x488C50` | `FillMatrix` | Reads the current matrix, overlays key codes and macro ids, and returns 1024 bytes |
| `0x487F20` | key-matrix read | Sends `84 D4`, waits, reads 1032 bytes and copies 1024 bytes of matrix |
| `0x487E60` | key-matrix sender | Sends `04 D4 00 40 00 00 00` + 1024 bytes |
| `0x48CF00` | `StMacro_To_HdMacro` | Encodes one macro: name header + 4-byte action records |
| `0x48D2D0` | macro buffer builder | Builds the `(offset, size)` index table and concatenated macro data |
| `0x487CB0` | profile read | Sends `83 B6`, waits ~700 ms, reads the 1032-byte feature report and copies out 254 bytes |
| `0x487C10` | profile save | Sends `03 B6` + 254-byte profile in a 1032-byte feature report |
| `0x488FA0` | profile patch | Sets bytes 4, 5, 6, `0x14`, `0x15` and the key map at `0x16` before a save |
| `0x488010` | `SetLedRgbTab` | Sends `B8` + RGB table, but only when the table is shorter than 1025 bytes; not usable for the 80-key F75 |
| `0x48DBC0` | `CDevG5KB::ApplySetting` | A newer protocol (63-byte colour planes, 10 KiB macros); shown only as a contrast, **not** used by the F75 |

`0x410A70` uses `0x1E` (30) as the sleep divisor and stores `MatrixLen` in a
single byte at `devState+0x2ff8`. `0x489080` is the F75 path because its
`CDev916KB` string at `0x60E508` is the one used by this profile.

### Deep-mode verification pass (2026-11-07)

The installer payload `app/OemDrv.exe` (SHA-256
`169d827903e9bc29acf3d10adebb35c79d98bd93128a711790baf3fa8e45e34e`) was opened
again in Deep mode. The strings below were found with `native_strings_find`,
their cross-references followed, and every function below was read with
`native_function_pseudocode` (Ghidra 12.1.4). This pass corrected four earlier
readings; the changes are marked **corrected** or **new**.

| Address | Evidence | Verified result |
|---|---|---|
| `0x410A70` | `ev_a81fd92c…` | `SleepUI` at `0x5FCDD4`, `SleepHW` at `0x5FCDE4`, the warning text `SleepUI!=SleepHW` at `0x5FCDF4`; `LedOpt%d` at `0x5FCA28`; `MatrixLen` at `0x5FD120`; `SleepTime` at `0x5FD150`; `DefLedIndex` at `0x5FD1B0`; `clrKeyDn` at `0x5FCDA4`. |
| `0x460A70` | `ev_3a83eec4…` | Comma-list parser: each value is converted with `_wcstoul` and truncated to one byte; at most `param_2` entries. It has no destination for `DebounceUI`/`DebounceHW`, so those lists are lost. |
| `0x490680` | `ev_299ec180…` | **corrected** 520-byte frame. The function loops over a **6-byte** payload in chunks of at most `0x200`; it does not send six 512-byte chunks. It calls `FUN_00405100(0x14)` — a 20 ms busy-wait — before the send, then waits 30 ms for the reply. |
| `0x489080` | `ev_671c22e3…` | Frame order and delays; two 1024-byte LED payloads start at `matrix[0]` and `matrix[1024]`; per-group `Sleep(0x1E)`. The profile save path is `Sleep(0x1E)` = 30 ms before `FUN_00487C10`; the 100 ms sleep belongs to `SetLedRgbTab`; `ApplySetting` ends with `Sleep(500)`. |
| `0x488FA0` | `ev_1c0fd731…` | **corrected** profile patch: byte 4 is the `GaoshouIndex == selected index` flag, byte 5 is the selected record's hardware effect id, byte 6 is `StartGp + group offset`. The per-key map at `0x16` is `byte1=(brightness<<4)|speed`, `byte0=direction or 7`, with the mask/`0x80` rule for direction-capable effects. |
| `0x487CB0` / `0x487C10` | `ev_41919139…` / `ev_d258e583…` | Read request `83 B6`, sleep from `devState+0x294`, read `0x407` bytes, accept reply `83`; save `03 B6` + 254 bytes. |
| `0x487DA0` / `0x4879F0` | `ev_4ed7a59c…` / `ev_108fbf2c…` | LED header `09 <cmd> 00 40 00 00 00` + 1024 payload; report id 6; three `HidD_SetFeature` attempts 200 ms apart. |
| `0x488C50` / `0x487E60` | `ev_10fd5544…` / `ev_2669012f…` | **new** key-matrix frame is `04 D4 00 40 00 00 00` + 1024 bytes, built from 132 key slots; `MatrixLen` is not this loop bound. |
| `0x4881F0` | `ev_f1872e98…` | **new** macro frame is `05 <id> 00 40 00 00 00` + 1024 bytes (command byte 5, not 9). |

Four concrete corrections that now drive the build:

1. **LED table split (0x489080).** The two 1024-byte payloads are
   `matrix[0..1024]` for `0xBC` and `matrix[1024..1890]` for `0xC0`. The
   disassembly at `0x489795` (`rep movsd`) copies from `matrix` and the two
   `lea edi,[esp+…]` values differ by exactly `0x400` (1024). There is **no**
   eight-byte lead-in; the old `matrix[0..1016]` split was a stack-offset
   misreading.
2. **Profile byte 4 (0x488FA0).** It is not a generic "custom matrix" flag.
   It is `1` when `GaoshouIndex` (18 in `KB.ini`) equals the selected LedOpt
   record index; with this profile that means UI effect 19. Byte 6 is
   `StartGp (0x20) + group offset`, not a preserved read value.
3. **`clrKeyDn` (0x410A70 + `KB.ini`).** Parser fallback is `0xFFFFFF`, but the
   F75 profile overrides it to `255,0,0` (red). The build now uses red.
4. **LED group timing (0x489080).** `Sleep(0x1E)` = 30 ms runs before each group,
   not only 50 ms between the two frames.

The same pass corrected four more values that were still estimates or wrong:

1. **Config pre-send delay (0x490680).** The call before `HidD_SetFeature` is
   `FUN_00405100(0x14)`, a QueryPerformanceCounter busy-wait, so it is **20 ms**
   (not the 30 ms used by the LED group loop).
2. **Profile save delay (0x489080).** The save is preceded by `Sleep(0x1E)` =
   **30 ms**. The 100 ms sleep happens only after `SetLedRgbTab`; it is not a
   profile-save delay. `ApplySetting` also ends with `Sleep(500)`.
3. **Per-key profile map (0x488FA0).** Byte 1 is `(brightness << 4) | speed`
   (not the reverse), and byte 0 is direction or `RandomVal=7`, with the
   `0x0F` mask and `0x80` up flag applied only when the selected effect supports
   direction. The build now writes all 80 keys.
4. **Loop bounds (0x489080/0x488C50).** The LED colour loop iterates the key
   count at `devState+0x2D28` (80); the key matrix builder iterates 132 slots.
   `MatrixLen` = 128 is neither of those bounds.

### Re-verified 2026-11-08

The payload was opened again as the active Deep target
(`sha256 169d8279…e34e`, same bytes as above). The strings were searched again
(`SleepUI` `ev_01b9f2c5…`, `SleepHW` `ev_3b54f063…`, `LedOpt` `ev_7def3d9e…`,
`MatrixLen` `ev_a39a5f27…`, `SleepTime` `ev_670d7807…`, `Gaoshou`
`ev_c5af8beb…`, `DefLedIndex` `ev_817302d1…`) and every referenced function was
read again with the Deep engine. The evidence ids are the same content-addressed
records as the 2026-11-07 pass, which confirms the earlier readings rather than
replacing them. The exact rules the build uses:

| Address | Evidence | Rule, as read |
|---|---|---|
| `0x490680` | `ev_299ec180…` | Frame is `b[0]=9`, `b[2]=5`, `b[3]=1`, `b[4]=1`, `b[5]=chunk`, `b[6]=len&0xFF`, `b[7]=len>>8`; payload is 6 bytes total. `b[1] = low byte of Σ b[2..0x207]`. `FUN_00405100(0x14)` runs first, then `WaitForSingleObject(…,0x1E)` = 30 ms; 3 attempts, `Sleep(0x3C)` = 60 ms apart. |
| `0x488FA0` | `ev_1c0fd731…` | For `i` in `0..N` (N = byte at `devState+0x2FF5`, 80 for F75): `profile[0x16+2i] = dir`, where `dir = RandomVal (default 7)` when the key has no direction, else the key's direction; if the selected LedOpt record supports direction, `dir = (dir & 0x0F) | (0x80 if dir == 2)`. `profile[0x17+2i] = (light << 4) | (speed & 0x0F)`. Bytes 4/5/6 and `0x14/0x15` as above. |
| `0x489080` | `ev_671c22e3…` | Colour word per key is written as low/mid/high byte into the three `0x7E` (126-byte) planes selected by `GameRGBIndex`; one group is `3 × 0x7E = 0x17A` (378) bytes and the buffer is `0x762` (1890) bytes. Loop bound is `devState+0x2D28` (80). Before each group `Sleep(0x1E)` = 30 ms; then `0xBC` from `matrix[0]`, `Sleep(0x32)` = 50 ms, `0xC0` from `matrix[1024]`, `Sleep(0x32)`; `ApplySetting` ends with `Sleep(500)`. |
| `0x487DA0` / `0x4879F0` | `ev_9e1c6167…` / `ev_108fbf2c…` | Header `09 cmd 00 40 00 00 00` + 1024 bytes; report id 6; `HidD_SetFeature` 3 times, `Sleep(200)` between attempts. |
| `0x487CB0` / `0x487C10` | `ev_41919139…` / `ev_d258e583…` | Read: `83 B6` request, `Sleep(devState+0x294)`, `HidD_GetFeature` 0x407 bytes, accept reply byte 0x83, copy 254 bytes. Save: `03 B6` + 254-byte profile, zero-padded to 1032. |
| `0x488C50` / `0x487E60` | `ev_10fd5544…` / `ev_2669012f…` | Key matrix `04 D4 00 40 00 00 00` + 1024 bytes from 132 slots (`0x84`), `Sleep(0x1E)` before. |
| `0x4881F0` | `ev_f1872e98…` | Macro frame `05 <id> 00 40 00 00 00` + 1024 bytes; ids `0xDC/0xE0/0xE4/0xE8`; caller sleeps `0x32` = 50 ms after each. |
| `0x405100` | `ev_ea950905…` | Milliseconds busy-wait: `QueryPerformanceCounter` loop until elapsed ≥ argument, so `0x14` = 20 ms. |

The build already consumes each of these (`app/src/model.rs`,
`app/src/device.rs`); this pass changed no constants. The values that remain
**estimated** are listed under "Open questions" below.

`FUN_00410A70` also seeds **13 built-in LedOpt records** at
`devState+0x3034` (stride `0x28`, enabled byte at `-1`, then UI id, hardware id,
speed, brightness, direction, random, colour) before reading `LedOpt1..24`.
The built-in UI/hardware pairs are `5/0, 7/1, 17/2, 12/3, 1/4, 3/5, 2/6, 19/7,
15/8, 13/9, 20/10, 16/11, 18/12`; slots 14..24 stay zero unless the profile
supplies them. The F75 file overrides slots 1..20.

### Key matrix and macro encoding (2026-11-08 pass)

This pass decoded the two payloads that the earlier build left local. Every
function below was read again from `OemDrv.exe`
(`sha256 169d8279…e34e`) with the Deep engine.

| Address | Evidence | Verified result |
|---|---|---|
| `0x487F20` | new record | Read request is `84 D4 00 00 00 00`, sent through `FUN_004879F0` (which prepends report id 6). It sleeps `devState+0x294`, calls `FUN_00487A80(…, 0x407)` and copies **256 dwords = 1024 bytes**. `FUN_00487A80` calls `HidD_GetFeature` with `0x407 + 1 = 1032` bytes and retries three times with `Sleep(200)`; the report id is not part of the copied matrix. |
| `0x487E60` | `ev_2669012f…` | Write frame is `04 D4 00 40 00 00 00` + 1024 bytes. `FUN_004879F0` prepends report id 6, so the wire frame is `[06 04 D4 00 40 00 00 00] + 1024`. |
| `0x488C50` | `ev_10fd5544…` | `FillMatrix`: 132 records (`0x84`), one **dword per matrix slot**, low bytes = keycode, high byte = macro id − 1. It first calls `0x487F20` so untouched keys keep their current code. A record with type byte `0x05` is a macro; a record whose slot byte is `0x02` uses the high byte as a layer index and writes at `+0x84`. |
| `0x48CF00` | new record | `StMacro_To_HdMacro`: one length byte (UTF-16 bytes, max `0x1e` = 30 units) + UTF-16LE name, then 4-byte actions. |
| `0x48D2D0` | new record | One 4-byte `(offset:u16, size:u16)` entry per bound macro at the start of the buffer; every offset is shifted by `4 * macro_count`; encoded macros follow. Total = `data + 4 * count`; the F75 budget is `MacroBufferSize = 0x1000` (4096). |

#### Key matrix rules

- Buffer: `1024` bytes = `256` dwords. `FUN_00488C50` rejects an index above
  `0x83` (131), so the first `132` dwords are addressable.
- Slot dword: low 24 bits carry the keycode (a Windows VK for ordinary keys,
  the KB.ini `0x02000000 | usage` form for consumer keys); the high byte
  carries `macro_id - 1` for a macro key.
- The driver's key records are 16 bytes; the matrix index is the byte at
  `record + 0x24`. `KB.ini` does not store that byte. For this build the
  physical order is the `[KEY]` order: `K1..K80` → matrix slots `0..79`
  (Esc = 0, F1 = 1, … PgDn = 79). This is an explicit assumption; the app
  reads the live matrix first and only overlays keys the person changed, so a
  wrong assumption cannot erase an untouched factory key.
- A macro key keeps the low 24 bits of the slot and replaces the high byte
  with `macro_id - 1`; ids are `1..=0x20` (`0x488C50`).

#### Macro record rules

The editor stores one `MacroStep`; the driver stores 12-byte action records
(`u16 code`, `u16 flags`, `u32 value`, `u16 param`). `FUN_0048CF00` encodes each
action as four bytes:

```
byte0 = (type << 4) | ((value >> 16) & 0x0F)
byte1 = (value >> 8) & 0xFF
byte2 = value & 0xFF
byte3 = HID code
```

- `value` is a 20-bit time offset (`0xFFFFF` cap). `FUN_004028A0` accumulates
  these values, so the offset is relative to the previous action, not an
  absolute timestamp.
- `flags` bit 0 = `1` means key-up; `0` means key-down. For types 1 and 2 the
  encoder sets bit 7 of `byte0` on key-down (`flags == 0`). Types 3, 4 and 5
  have no down/up bit.
- The 2026-11-08 build folds a macro step's `delay_ms` into the next action's
  `value`, and emits a down/up pair separated by `holding_ms`; a standalone
  `Delay` step adds to the following action.
- Types: `1` keyboard, `2` mouse button, `3` and `4` the vendor's
  `VK 0xF8/0xF9` actions (parameter in `byte3`), `5` wheel
  (`VK 0xF5` positive, `VK 0xF6` negative).
- Modifier special cases: `VK 0x10/0xA0 → HID 0xE1`, `0x11/0xA2 → 0xE0`,
  `0x12/0xA4 → 0xE2`, `0x5B → 0xE3`, `0x5C → 0xE7`, `0xA1 → 0xE5`,
  `0xA3 → 0xE4`, `0xA5 → 0xE6`.
- Mouse buttons: `VK 0xF0..0xF4 → HID 1, 2, 4, 8, 0x10`, type 2. A device
  with exactly one HID item swaps `VK 0xF3/0xF4` to HID `0x10/0x08`.
- All other keys go through the 115-entry `(HID, VK)` table at VA `0x607290`
  (file offset `0x206490`); the table was read byte-for-byte from the PE and
  is in `model.rs` as `VK_TO_HID`. A missing pair logs
  `StMacro_To_HdMacro: get wrong hid` and the action is dropped.
- Macro name: `buffer[0] = name_bytes`, then UTF-16LE name; `0x48CF00` uses
  `_wcsnlen(name, 0x1e)`, so at most 30 UTF-16 units (60 bytes).

#### Macro upload rules

- Buffer layout: `[count × (offset:u16, size:u16)] [macro 1] [macro 2] …`.
  Each `offset` already includes the `4 × count` header; this is exactly the
  shift `FUN_0048D2D0` applies before copying the table to the front.
- Upload in four `0x400` (1024) byte chunks with ids
  `0xDC, 0xE0, 0xE4, 0xE8`; frame `05 <id> 00 40 00 00 00` + 1024, report id 6
  prepended. `FUN_00489080` sleeps `0x32` (50 ms) after each chunk.
- The key matrix is sent after the macro chunks and before the profile; the
  vendor order is macro (bit 0) → key matrix → profile/lighting (bit 1).
- Binding: the macro's trigger key must be one of the decoded choices F9..F12
  (`KEYS` indices 9..12); the builder writes `macro_id - 1` into that slot's
  high byte. “Mouse 4/5” and “Not assigned” remain local.

## Device identity

| Value | Setting | Where |
|---|---|---|
| Wired USB | VID `0x258A`, PID `0x010C` | `KB.ini` `VID`, `PID` |
| 2.4 GHz | VID `0x3554`, PID `0xFA09` | `KB.ini` `VID_Wireless`, `PID_Wireless` |
| Name | AULA F75 KR / F75 KR | `KB.ini` `Name`, `ShortName` |
| Firmware | 24 (`0x18`) | `KB.ini` `Fw=24` |
| Key rectangles | 80 keys + wheel | `KB.ini` `[KEY]` `K1..K81` |
| LED indices | 0..89 (90 LEDs) | `KB.ini` `[KEY]` last field |
| MatrixLen | 128 key slots | `KB.ini` `MatrixLen=128`; `FUN_00410a70` stores it at `devState+0x2ff8`. The LED group loop uses the separate key count at `devState+0x2d28` (80 for the F75), so MatrixLen is not that loop bound. |
| Channels | 3 (`ChannelMask=3`, wireless + wired) | `KB.ini` |
| Sleep timeout | 300 s | `KB.ini` `SleepTime=300` |
| Battery display | off (`ShowPower=0`) | `KB.ini` |

`K81` is the volume wheel: dialogue type `0x09`, code `0x01`, usage
`0x0700001D`, LED 84. It is drawn as the small wheel on the right edge.

## Key layout

`KB.ini` stores each key as `x1,y1,x2,y2,type,keycode,modifier,led`.
`type 0x02` is a normal key, `0x09` is the wheel. Coordinates use a 600×233
diagram. The preview scales that rectangle, so key positions and widths match
the vendor picture without copying it.

Examples:

| Key | Rect | VK | LED |
|---|---|---|---|
| Esc | 30,30,55,57 | `0x1B` | 0 |
| F1 | 107,30,132,57 | `0x70` | 12 |
| Tab | 34,120,78,147 | `0x09` | 2 |
| Caps | 33,158,85,185 | `0x14` | 3 |
| Space | 174,235,400,262 | `0x20` | 35 |
| Fn | 414,235,449,262 | `0xFA` | 53 |
| Right | 605,234,630,261 | `0x27` | 89 |

The FN layer is a separate table (`[FN1]`) of 32-bit HID usages, for example
`0x07000004` (consumer page), `0x02000223`, and `0x0803xxxx` (vendor page).
Keyboard usages are ordinary HID keyboard usages (`A` = `0x04` on the wire);
`KB.ini` keeps the Windows virtual-key code instead (A = `0x41`).

## Lighting

`Light`/`LightHW` are 0..9 (10 brightness steps), `Speed`/`SpeedHW` are 0..4
(5 speeds). `RGBIndex=0,1,2` maps the three colour channels in R,G,B order.
If `RGBIndex`/`GameRGBIndex` are absent, `FUN_00410A70` writes the **reversed**
order `2,1,0` (`devState+0x2FFC..+0x3001`); a present list must sum to 3 or the
parser raises "R G B Index Error" / "clrGmKey RGB Index Error". The F75 profile
supplies `0,1,2` for both.
`DefLedIndex=10` is the default effect. Reading the zero-based list puts entry
10 at **Neon stream**, so that is the page's starting effect. This is an
interpretation of the index (there is no separate default-effect string in the
payload).

`FUN_00410A70` parses up to **24** `LedOpt1..LedOpt24` records into 0x28-byte
structures at `devState+0x3034`; the F75 `KB.ini` ships 20. Byte 0 of each
record is the enabled flag. The seven numbers per line are: effect id,
hardware effect id, speed support, brightness support, direction support,
random-colour support, colour support.

Two lines need care. `LedOpt19=21,0,0,0,0,0,0` and
`LedOpt20=0,0,0,0,0,0,0`: the raw file therefore has a self-define-style entry
with UI id 21 / hardware id 0, and an all-zero entry. The app's "Self-define"
and "Off" names are UI labels, not strings recovered from the executable (the
effect display names are localized outside `OemDrv.exe`).

When the paired `Light`/`Speed` lists are absent, the parser's `LedParam`
default sets the light byte to `nLightNum-1` and the speed nibble to
`(nSpeedNum-1)*16 | 7`. The shipped profile supplies explicit lists, so the
app uses 10 brightness levels and 5 speed levels.

| Effect | HW id | Speed | Bright | Direction | Random | Colour |
|---|---|---|---|---|---|---|
| Fixed on | 1 | – | ✓ | – | ✓ | ✓ |
| Respire | 3 | ✓ | ✓ | – | ✓ | ✓ |
| Rainbow | 2 | ✓ | ✓ | – | – | – |
| Flash away | 19 | ✓ | ✓ | – | ✓ | ✓ |
| Raindrops | 15 | ✓ | ✓ | – | ✓ | ✓ |
| Rainbow wheel | 13 | ✓ | ✓ | – | ✓ | ✓ |
| Ripples shining | 20 | ✓ | ✓ | – | ✓ | ✓ |
| Stars twinkle | 16 | ✓ | ✓ | – | ✓ | ✓ |
| Shadow disappear | 18 | ✓ | ✓ | – | ✓ | ✓ |
| Retro snake | 5 | ✓ | ✓ | – | ✓ | ✓ |
| Neon stream | 7 | ✓ | ✓ | – | ✓ | ✓ |
| Reaction | 17 | ✓ | ✓ | – | ✓ | ✓ |
| Sine wave | 12 | ✓ | ✓ | – | ✓ | ✓ |
| Retinue scanning | 8 | ✓ | ✓ | – | ✓ | ✓ |
| Rotating windmill | 28 | ✓ | ✓ | – | – | – |
| Colorful waterfall | 30 | ✓ | ✓ | – | – | – |
| Blossoming | 14 | ✓ | ✓ | – | – | – |
| Rotating storm | 29 | ✓ | ✓ | – | ✓ | ✓ |
| Self-define | 0 | – | – | – | – | – |
| Off | 0 | – | – | – | – | – |

Direction is a four-way flag (left, right, up, down). `LedMask=0x22120` and
`GaoshouIndex=18` describe the side strip; the UI only exposes an on/off switch,
which is an honest simplification of an undecoded side-LED map.

The preview colours are generated locally by `led_color` (one animation per
effect). They approximate the hardware look; they are not a copy of vendor
bitmaps.

## Sleep and debounce

`FUN_00410a70` (KB.ini parser) reads the paired `SleepUI`/`SleepHW` lists and
uses the device value `UI seconds / 30`. The profile lists agree exactly:

```
UI seconds     60  90 120 180 240 300 600 900 1200
device units    2   3   4   6   8  10  20  30   40
```

If `SleepUI` is missing, the parser falls back to
`30,60,90,120,180,240,300,600,900,1200` s and derives `1,2,3,4,6,8,10,20,30,40`
device units. A present `SleepUI` is only checked against `SleepHW` by count;
a mismatch shows the message box "SleepUI!=SleepHW" and the hardware list is
still used. `SleepTime` defaults to `0x78` (120 s) when the key is absent; the
F75 profile sets 300 s. The UI shows both the chosen seconds and the device
value it will send.

`ShowDebounce=0x04000101` packs four stages (`01 01 00 04`). `FUN_00410A70`
stores the packed value and reads optional `DebounceUI`/`DebounceHW` lists, but
the list parser (`FUN_00460A70`) has no destination for them, so the packed
field is all that reaches `devState`. There is no millisecond table in the
payload and the stage timing is not recovered. This app therefore
**estimates** 1/2/4/8 ms for the four stages and defaults to the second one.
Marked `estimated` in `mechanics.json`.

## Macros

The parser defaults `MacroBufferSize` to `0x1000` = 4096 bytes (also present in
the profile). Macro actions carry a delay and a hold time; playback can be
limited by cycle count, by releasing the hotkey, or by any key. The editor now
uses the **encoded** size from `0x48CF00`: one length byte + UTF-16LE name (max
30 units), plus 8 bytes for each key/mouse step (down + up) and 8 bytes per
character of a text step. A delay has no record of its own; it is folded into
the next action's 20-bit time value.

`FUN_00489080` uploads the assembled buffer through `FUN_004881F0` in four
`0x400` (1024) byte chunks with ids `0xDC`, `0xE0`, `0xE4`, `0xE8`, then waits
50 ms. The frame is `[06 05 <id> 00 40 00 00 00] + 1024 data bytes`: the command
byte is `5` and the macro id sits where the LED command sits (byte 2 after the
report id), not in a `0x09` frame. The buffer starts with one 4-byte
`(offset, size)` entry per macro, already shifted by `4 * count`.

`FUN_00488C50` builds the key matrix in a 1024-byte buffer from 132 slots
(`0x84`); `FUN_00487E60` sends it as `[06 04 D4 00 40 00 00 00] + 1024 bytes`.
`MatrixLen` (128) is used elsewhere in the parser; the key scan and the LED
colour loop use the key count (80) separately. The app reads the live matrix
with `FUN_00487F20` (`84 D4`) before overlaying only the assignments the person
made, so a sparse key map is safe.

## Parser defaults and profile rules

`FUN_00410A70` writes these values into `devState` before applying the profile:

| Setting | Default | Profile value |
|---|---|---|
| `MacroBufferSize` | `0x1000` (4096) | 4096 |
| `MaxFnNum` | `0x30` (48) | — |
| `MatrixLen` | `0x80` (128) | 128 |
| `CRC` | 1 | 1 |
| `SleepTime` | `0x78` (120 s) | 300 s |
| `DefLedIndex` | 0 (`devState+0x2F1F`) | 10 |
| `RGBIndex`, `GameRGBIndex` | `2,1,0` if absent | `0,1,2` |
| `GaoshouIndex` | -1 (`devState+0x2F20`) | 18 |
| `GaoshouGroupNum` | 5 (`devState+0x2FF0`) | 1 |
| `StartGp` | `0x20` | — |
| `clrKeyOv` | `0xFFFFFF` | `255,255,255` |
| `clrKeyDn` | `0xFFFFFF` | `255,0,0` (profile override) |
| `Light` count | 20 (`1..20`) | 10 (`0..9`) |
| `Speed` count | 4 (`1..4`) | 5 (`0..4`) |
| `clrKeyHasFunc` | `0x1D45E8` (10,147,255) | same |
| `Gaoshou` colour | `0xFFFFFF` | `255,0,0` |
| `FnNotToSet` | 1 | 0 |
| `ShowSideLED` | 0 | — |
| `ShowPower` | 0 | 0 |

Other rules read from the same function:

- `GaoshouKey%d` is read once per group into
  `0x28`-byte records; `GaoshouGroupNum` therefore controls both the group
  count and the number of key records.
- Up to 20 `KeyNotToSet` entries and four `KeyNotToSetColor` entries are read.
- `SpeedUI`/`SpeedHW` and `LightUI`/`LightHW` are only checked for equal counts;
  a mismatch shows a message box and the hardware list is used.
- DPI blocks validate the range (`Wrong DPIRANGE`), require equal
  `DPISET`/`DPI` counts, and reject a default DPI that is not in the list.
- `SideLedOpt1..5` are parsed into `0x28`-byte records at `devState+0x33F5`
  (the enabled byte is at `-1`) only when `ShowSideLED` is set.

## USB / HID transport (research notes)

The vendor applies settings through `CDev916KB::ApplySetting` at
`FUN_00489080`. Its flag word selects the work:

- bit 0 (`0x01`): fill and send the key matrix (`0x04 0xD4`), then the macro
  buffers.
- bit 1 (`0x02`): read the 254-byte profile, run `FUN_00488FA0`, and save it.
  This is the branch the app uses.
- bit 4 (`0x10`): send the 1890-byte LED colour matrix in two `0x09` frames,
  once per `GaoshouGroupNum` group.
- bit 6 (`0x40`): send the compressed per-key RGB table (`SetLedRgbTab`), but
  only when `GaoshouIndex != selected index`. It writes three bytes per key
  (R/G/B planes), length `(count+1)*0x15`, waits 20 ms before the send and
  100 ms after; the F75's 80 keys exceed its 1025-byte limit, so this build
  does not use that path.

The order inside `ApplySetting` is: global `Sleep(0x1E)` = 30 ms; bit 0
(key matrix and macros in `0x400`-byte buffers with ids
`0xDC/0xE0/0xE4/0xE8`, `Sleep(0x32)` = 50 ms after each); bit 1 (profile
read/patch, `Sleep(0x1E)` = 30 ms, save); bit 4 (LED colour matrix); bit 6
(compressed RGB table, `Sleep(0x64)` = 100 ms after it). The function ends
with `Sleep(500)`.

The LED colour matrix is `0x762` = 1890 bytes: five groups of `0x17A` = 378
bytes, each group three `0x7E` = 126-byte planes. For every key record up to
the key count at `devState+0x2D28` (80 for the F75), the LED index comes from
the key record at `+0xD`, and the colour
is the 32-bit value at `devState + (0x912 + keyIndex)*4`. The low byte (R) goes
to plane `GameRGBIndex[0]*126`, the next (G) to `GameRGBIndex[1]*126`, and the
high (B) to `GameRGBIndex[2]*126`. `GaoshouGroupNum` is 1 in the F75 profile,
so one 378-byte group is filled. The vendor code waits 30 ms, sends `0xBC`
(matrix), waits 50 ms, sends `0xC0` (colours), then waits 50 ms. The 1890 bytes
do not fit in one payload, so `FUN_00487DA0` sends two 1024-byte chunks:
`0xBC` carries matrix[0..1024] and `0xC0` carries matrix[1024..1890] padded with
zeros. Both frames are `[06 09 <cmd> 00 40 00 00 00] + 1024 data bytes`. This
build sends the two chunks for the self-defined effect (UI 19). The split is
`LED_MATRIX_FIRST_CHUNK = 1024`; the earlier 8 + 1016 split was corrected after
reading the two `lea edi` pointers in the disassembly of `FUN_00489080`
(`0x4897A5` and `0x4897EB`), which differ by exactly `0x400`.

Two different feature frames are used:

- `FUN_00490680` builds the **520-byte config frame** (`0x208`):
  `[0]=9`, `[1]=checksum`, `[2]=5` (command), `[3]=1`, `[4]=1`,
  `[5]=chunk/index counter`, `[6]=payload length low byte`,
  `[7]=payload length high byte` (little-endian), payload from `[8]`.
  The checksum is the low byte of the sum of bytes `[2..0x207]`.
  `HidD_SetFeature` sends the frame after `FUN_00405100(0x14)`, a 20 ms
  QueryPerformanceCounter busy-wait, then the code waits `0x1E` = 30 ms for
  the reply, calls `HidD_GetFeature` for the `0x208` reply and compares the
  command byte; it retries three times, 60 ms apart
  (`AccessData: ... nCmdID=%x`). The loop
  starts at payload offset 0 and advances by `min(0x200, 6 - offset)`, so the
  F75 path is **one frame** with chunk index `[5]=0` and a **6-byte** payload
  (`[6..7]=6,0`). The earlier "six 512-byte chunks" note is wrong; there are
  six payload bytes, not six chunks. Errors `0x48F` and `6` abort immediately;
  a reply command byte other than `5` is rejected.
- `FUN_00487DA0` builds the **1032-byte LED frame**: header
  `09 <cmd> 00 40 00 00 00` followed by a 1024-byte payload.
  `FUN_004879F0` prepends report id `6`, copies the payload and calls
  `HidD_SetFeature` up to three times, 200 ms apart. The header's `0x40` is a
  length/marker field, not a checksum; only the 520-byte config frame has the
  sum checksum.

### Profile read and save

The lighting profile is **254 bytes**, separate from the 520-byte config frame.
`FUN_00487CB0` sends a 1032-byte feature report `[06 83 B6 00 …]`, sleeps for the
vendor's read delay (about 700 ms), calls `HidD_GetFeature`, expects `83 B6` and
copies 254 bytes out. `FUN_00487C10` sends the same report with `[06 03 B6]` and
the 254 profile bytes, zero-padded to 1032.

Before saving, `FUN_00488FA0` patches the profile read from the keyboard:

| Offset | Value |
|---|---|
| `4` | `u8(GaoshouIndex == selected LedOpt index)` — `GaoshouIndex=18` in `KB.ini`, so it is `1` for the 0-based index 18 (UI effect 19, Self-define) and `0` otherwise |
| `5` | `LedOpt[selected].hw` — the hardware effect id (for example Neon stream = 7) |
| `6` | `StartGp (0x20) + group offset`; the F75 uses one group, offset 0, so the byte is `0x20` |
| `0x14`, `0x15` | `0xFF`, `0xFF` |
| `0x16…` | two bytes per key: byte 1 is `(brightness << 4) | speed`; byte 0 is direction or `RandomVal=7`, with the direction mask/0x80 rule below |

The pseudocode gives the exact per-key rule. For each of the 80 key records,
`FUN_00488FA0` writes two bytes at `profile + 0x16 + key*2`:

- **byte 1** = `(light_index << 4) | (speed_index & 0x0F)`. The indices are the
  UI values from the `LightHW`/`SpeedHW` tables (`devState+0x2e54` and
  `devState+0x2e74`). For the F75 those tables are `0..9` brightness and
  `0..4` speed, so a default setting of brightness 7 and speed 2 writes
  `0x72`.
- **byte 0** = the key's direction; if the key has random colour enabled, it is
  `RandomVal` (the parser default is `7`). When the selected `LedOpt` record
  supports direction, the vendor masks it with `0x0F` and, for direction `2`
  (up), sets bit `0x80`. All 20 F75 `LedOpt` records have direction support
  `0`, so this profile writes the unmasked value.

`patch_profile` now applies this rule to every key using the app's global
brightness, speed, direction and random-colour controls. The vendor keeps
per-key brightness/speed/direction/random fields; the app currently exposes
them globally, which is a UI simplification, not a protocol difference.

This build reads the profile, patches bytes 4, 5, 6, `0x14`, `0x15` and the
`0x16` key map, and writes it back. That is the path that changes the effect on
the keyboard. The vendor only does this when its Save button is pressed
(`ApplyNow=0`), so an explicit **Apply to keyboard** button is required.

The global bytes `7..0x13` are still not mapped. They do not carry the per-key
brightness/speed values; those are fully covered by the `0x16` table. A USB
capture of a global brightness or speed change would still be useful to name
them, but the build no longer has to guess the per-key values.

Timing constants recovered from these functions (also in `model.rs` and
`device.rs`): **20 ms** before a config frame (`FUN_00405100(0x14)`, a busy
wait), 60 ms between config retries, 50 ms after each macro/key-matrix/LED
command, 100 ms only after `SetLedRgbTab` (the compressed RGB table), 30 ms
before the profile save (`Sleep(0x1E)`), 500 ms at the end of `ApplySetting`,
and 200 ms between LED send retries.

## Application layout

The vendor window is 1200×760, dark, with a 64 px top bar and a narrow icon rail
(`Cfg.ini`: text 180,180,180; accent 10,147,255; menu 37,37,37; item highlight
0,93,145). This rebuild keeps that structure and palette but uses its own
vector logo, icons and keyboard drawing — no vendor photos, logos or fonts.

Pages: Lighting, Key assignment, Macros, Music, Profiles, Settings, Device,
Documentation. Profiles are local JSON files under `~/.config/f75-control/`,
with import and export. Five slots are a product decision (**estimated**), not
a value read from the payload. Music mode is a simulated preview; the keyboard
reacts on its own and the vendor driver stores only the mode and sensitivity.
The Documentation page carries the short user guide; `README.md` is the full
one, and this file stays the protocol reference.

## Build constants after the verification pass

These are the values the Rust build now consumes (`app/src/model.rs`,
`app/src/device.rs`):

| Constant | Value | Source |
|---|---|---|
| `LED_MATRIX_FIRST_CHUNK` | `1024` | `FUN_00489080` `0x4897A5`/`0x4897EB`; second frame starts at `matrix+0x400` |
| `GAME_RGB_INDEX` | `[0, 1, 2]` | `KB.ini GameRGBIndex`; `FUN_00489080` planes at `0x2FFF..0x3001` |
| `GAOSHOU_INDEX` | `18` | `KB.ini GaoshouIndex`; `FUN_00488FA0` profile byte 4 |
| `START_GP` | `0x20` | `FUN_00410A70` default and `FUN_00488FA0` byte 6 |
| `CLR_KEY_DN` | `[255, 0, 0]` | `KB.ini clrKeyDn` override (parser fallback is white) |
| `LED_GROUP_DELAY` | `30 ms` | `FUN_00489080` `Sleep(0x1E)` before each group |
| `LED_MATRIX_BYTES` / `LED_COLOR_PLANE_BYTES` | `1890` / `126` | `FUN_00489080` `0x762` buffer, `0x7E` planes |
| `MATRIX_DELAY` | `50 ms` | `FUN_00489080` `Sleep(0x32)` between `0xBC`/`0xC0` and after |
| `CONFIG_CHUNK_SIZE` | `512` (`0x200`) | `FUN_00490680` caps one payload chunk at `0x200` |
| `CONFIG_FRAME_DELAY_MS` / `CONFIG_RETRY_DELAY_MS` | `20` / `60` | `FUN_00490680` `FUN_00405100(0x14)` pre-send busy-wait / `Sleep(0x3C)` between retries |
| `READ_DELAY` | `700 ms` | `FUN_00487CB0` reads `devState+0x294`; still **estimated** because no setter was located |
| `SAVE_DELAY` | `30 ms` | `FUN_00489080` `Sleep(0x1E)` before `FUN_00487C10` |
| `APPLY_SETTLE_DELAY` | `500 ms` | `FUN_00489080` final `Sleep(500)` |
| `RGB_TABLE_DELAY_MS` | `100 ms` | `FUN_00489080` `Sleep(0x64)` after `SetLedRgbTab` only |
| `PROFILE_KEY_MAP_OFFSET` / `PROFILE_KEY_STRIDE` | `0x16` / `2` | `FUN_00488FA0` per-key table |
| `PROFILE_KEY_COUNT` | `80` | `FUN_00488FA0` key count at `devState+0x2FF5`; F75 profile |
| `KEY_MATRIX_BYTES` | `1024` | `FUN_00487E60`/`FUN_00487F20` copy `0x100` dwords |
| `KEY_MATRIX_SLOTS` | `132` | `FUN_00488C50` loop bound `0x84` |
| `KEY_MATRIX_READ_HEADER` | `84 D4 00 00 00 00` | `FUN_00487F20` |
| `KEY_MATRIX_WRITE_HEADER` | `04 D4 00 40 00 00 00` | `FUN_00487E60` |
| `MACRO_INDEX_ENTRY_BYTES` | `4` | `FUN_0048D2D0` `(offset:u16, size:u16)` |
| `MACRO_NAME_MAX_CHARS` | `30` | `FUN_0048CF00` `_wcsnlen(…, 0x1e)` |
| `MACRO_MAX_ID` | `32` | `FUN_00488C50` accepts ids `1..=0x20` |
| `VK_TO_HID` | `115` pairs | `FUN_0048CF00` table at VA `0x607290` |

## Linux access to the keyboard (observed on this host)

This part is not from the Windows program; it is what Linux exposes for the
same keyboard and it explains the `os error 13` message.

- The 2.4 GHz receiver `3554:fa09` creates two hidraw nodes. `hidraw5`
  (`input0`) is the plain keyboard interface (usage page 1, usage 6) and has no
  vendor feature report. `hidraw6` (`input1`) is the control interface: vendor
  usage page `0xFF02` report id `0x13` (19-byte input/output) plus vendor usage
  page `0xFF04` report id 6 declared as a **7-byte feature** item
  (`85 06 ... 95 07 B1 02`).
- Both nodes are `0600 root:root` by default, so a normal user gets
  `EACCES` (`os error 13`) when opening them. The app now reads the report
  descriptor from `/sys/class/hidraw/*/device/report_descriptor`, marks the
  node with feature report id 6 as the control interface, and sends settings
  there; the plain keyboard node is skipped when a control node exists.
- **The `uaccess` tag alone was not enough on this host.** The rule from the
  first attempt (`MODE="0660", TAG+="uaccess"`) was installed as
  `/etc/udev/rules.d/99-f75.rules`. udev reads rules in lexical order, and the
  ACL is scheduled by `73-seat-late.rules` (`TAG=="uaccess", RUN{builtin}+="uaccess"`);
  a `99-` file is read *after* it, so the tag was set too late and the builtin
  never ran. `udevadm info` showed `TAGS=:uaccess:seat:`, but `getfacl` on both
  nodes still reported only `user::rw-`, `group::rw-`, `other::---` — no
  `user:pbolk` ACL. With group `root`, mode `0660` therefore still denied the
  logged-in user. This is the concrete reason the copied `sudo` command
  "succeeded" yet access did not appear.
- The fixed rule is written to **`70-f75.rules`** (before `73-…`) and removes
  the old `99-f75.rules`; each line also names `GROUP="<primary group>"`, so
  access does not depend on the seat/ACL path at all. `udevadm verify` accepts
  it (systemd 259 only warns that ownership by a non-system group is slated for
  deprecation; the ACL path remains as the forward-compatible half). The Device
  page can run the saved script through `pkexec` (one system password dialog,
  app stays unprivileged) and falls back to a one-line command or a script.
  Unit tests check that the generated command is valid shell and contains both
  IDs, the group and the removal of the old file.
- **Observed receiver behaviour (read-only probes, root).** On `hidraw6`,
  `HIDIOCGFEATURE` for report id 6 at length **1032 succeeds** (the descriptor
  only declares 7 bytes) and returns `06 00 02 64 64 64 99 24 65 …`, whose
  header is not `[06 83 B6]`; `HIDIOCGFEATURE` for report 0x13 also succeeds.
  On `hidraw5` every GET fails with `EPIPE`.
- **Observed write behaviour.** `HIDIOCSFEATURE` with `[06 83 B6 …]` at sizes
  1032, 8, 9, 16 and 20 all fail with `ETIMEDOUT` (`os error 110`) after about
  **5.1 s**. So the receiver accepts reads but does not answer the settings
  request in its current state (keyboard link asleep, or the longer frame is
  not accepted over 2.4 GHz). The app reports this as a timeout, recommends the
  USB cable, and runs the whole write on a worker thread so the window never
  freezes for those five seconds.

## Logging (recovered from `OemDrv.exe`)

The vendor app has a small logger of its own, selected once at startup. It is
not the Windows event log and it does not use a logging library.

| Step | Address | Behaviour |
| --- | --- | --- |
| Read setting | `0x40E2D0` | `GetPrivateProfileIntW("OPT","Debug",0,Cfg.ini)` → `DAT_0064946C`; `UIDebug` (default 0) → `DAT_006441E4`; then calls `FUN_00407770` |
| Initialise | `0x407770` | `GetLocalTime`; builds `%s\Log.txt` twice, once with `SHGetSpecialFolderPathW` and once with `SHGetSpecialFolderPathA` (nFolder `0` = desktop); installs debug-output writers by default |
| Turn file on | `0x407770` | only when `DAT_0064946C > 1`: opens `Log.txt` with `"w"` (truncates), then swaps both function pointers to the file writers |
| Wide file writer | `0x407580` | `_wfopen_s(..., L"a")`, `GetLocalTime`, writes `"\n%02u:%02u.%03u   "` then `_vfwprintf`, `_fclose` |
| ANSI file writer | `0x4076C0` | same with `_fopen_s(..., "a")`, `0x5FAB08` format string and `_vfprintf` |
| Wide debug writer | `0x4074F0` | when `Debug != 0`, prefixes `"BY  "`, formats into 512 wchars and calls `OutputDebugStringW` |
| ANSI debug writer | `0x407630` | same into an 8192-byte buffer, `OutputDebugStringA` |

The format strings are `"\n%02u:%02u.%03u   "` at `0x5FAAC0` (wide) and
`0x5FAB08` (ANSI): minutes, seconds and milliseconds, three spaces, then the
message. A shipped `Cfg.ini` has `Debug=0` and no `UIDebug`; therefore a normal
vendor launch writes nothing. `Debug=1` means debugger output only, and
`Debug=2` or more means a truncating `Log.txt` plus appended lines.

Calls go through the function pointers `DAT_00649470` (wide) and
`DAT_00649474` (ANSI), for example in `FUN_0040C530`:

```
OnNewDongle: Find new wireless, vid_pid=%04x_%04x, bBT=%x, nPsd=%02x,...
OnNewDongle: it's unsupported device
OnNewDongle: CHidDev object has init, ignore this dongle
OnNewDongle: new error
```

The apply path logs its failures with the same mechanism: `SetMacroData(%x)
error, err=0x%x` at `0x60E5A0`, `SetKeyMatrix error, err=0x%x` at `0x60E658`,
`SetLedRgbTab error` at `0x60E6F4` and `SetProfile error, err=0x%x` at
`0x60E7C8` (call sites in `FUN_00489080`). Other recovered messages cover
profile/macro open and read failures, `Data parse error!`, `Init_KeyUnit
Error`, `R G B Index Error`, `clrGmKey RGB Index Error`, `Wrong DPIRANGE` and
`SleepUI!=SleepHW`.

### Linux mapping

| Vendor | This app |
| --- | --- |
| `Debug=0` | level 0, off |
| `Debug=1`, `OutputDebugString` | level 1, errors only, echoed to stderr |
| `Debug>1`, `Log.txt` | levels 2 and 3, ring buffer plus `~/.config/f75-control/Log.txt` |
| Desktop folder | `~/.config/f75-control/` |
| Truncate at launch | the file is recreated at launch (a file over 1 MiB is first renamed to `Log.1.txt`) |
| `\nMM:SS.mmm   ` | `HH:MM:SS.mmm   `: the hour is added, the three-space separator is kept |
| `"BY  "` debug prefix | not needed; stderr carries level 1 |

Everything in this table is implemented in `src/log.rs`. The in-memory ring
(2000 lines), rotation at 1 MiB, four UI levels and the diagnostics export are
additions, not vendor behaviour; they are marked estimated in
`mechanics.json`. A panic hook writes the message and source location before
the process exits. Every apply, access check, probe, profile import/export and
settings write records its outcome with module and level, so a failed device
write can be explained after the window is closed.

## Open questions (estimated or hardware-only)

- **Debounce stage milliseconds** are `1/2/4/8 ms` marked estimated.
  `FUN_00410A70` parses `DebounceUI`/`DebounceHW` with `FUN_00460A70` but has no
  destination for the lists, and `FUN_0040F330` discards them too; the F75
  profile carries only the packed `ShowDebounce=0x04000101`. Needs a device read
  or USB capture.
- **`READ_DELAY` = 700 ms** comes from `Sleep(devState+0x294)` in
  `FUN_00487CB0`; no setter for `+0x294` was located, so the value is estimated.
- **2.4 GHz receiver**: the control interface declares a 7-byte feature report
  while the vendor frame is 1032 bytes. Reads at 1032 bytes work; a settings
  request times out after ~5.1 s on this host. Whether the wired path writes
  successfully is untested (no cable was connected); USB stays the proven path.
- **Receiver profile read header**: the 1032-byte GET returns a buffer starting
  `06 00 02 64 64 64 …`, not `[06 83 B6, profile…]`. Whether this is a stale or
  unsolicited report, or the dongle's reply format, is unresolved.
- **`DefLedIndex=10`** is treated as a 0-based index into the effect list, i.e.
  Neon stream. No instruction reads the byte at `devState+0x2F1F` directly
  (a `disp32 0x2f1f` byte search found no reader), so the interpretation stays
  documented rather than proven.
- **Profile bytes `7..0x13`** remain unmapped.
- **Key-record → matrix-slot byte** (`record + 0x24`) is not in `KB.ini`.
  The build assumes `K1..K80` → slots `0..79` and reads the live matrix before
  overlaying, so untouched keys keep their factory code. A wrong slot for a
  remapped key would move that one key, not disable the keyboard; verify on
  hardware before calling the mapping exact.

## Not implemented

- **Command/combination assignments** (`KEY_CATEGORIES` 3 and 4) and
  **“Double click”/“Back”/“Forward”** — the driver has action codes for them,
  but the corresponding VK/usage values were not recovered. They stay in the
  app and are skipped with a warning rather than sent as a wrong code.
- **FN-layer remaps** — `KB.ini [FN1]` confirms the 32-bit
  `0x02000000/0x07000000/0x08000000` form, but the driver's FN record →
  matrix-slot mapping was not decoded. Only the base layer is overlaid.
- **Music visualiser stream** — `FUN_0047F250` builds an output-report-style
  frame (three colour bytes, key count, key indices) and calls a virtual device
  method; it is a separate real-time protocol, not the feature-report path.
  The page stays a local preview.
- **Firmware writing** — no firmware functions were found in `OemDrv.exe`; the
  firmware panel is offline.
- The global profile bytes `7..0x13` (if they carry any global colour/speed
  state) — their meaning is unknown. The per-key brightness, speed, direction
  and random values **are** written at `0x16`, so those controls work.
- Account / cloud sharing / online firmware download — the vendor server is not
  something this app should fake.
