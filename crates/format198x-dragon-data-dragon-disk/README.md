# format198x-dragon-data-dragon-disk

Read and write **Dragon VDK disk images** in Rust.

A VDK is a small `dk` header followed by raw 256-byte DragonDOS sectors in track order. The crate is dependency-free and returns typed errors for malformed input.

## Provenance

Graduated from Emu198x's `format-dragon-disk` crate (emu198x/emu198x@63fac19e77), whose history of 9 commits stays in that repository. The name carries the manufacturer, Dragon Data, as well as the machine, as `{manufacturer}-{system}` requires; Emu198x's name had only the machine.

## Licence

GPL-2.0-or-later.
