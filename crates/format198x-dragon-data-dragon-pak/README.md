# format198x-dragon-data-dragon-pak

Read and write **Dragon cartridge and PC-Dragon PAK images** in Rust.

Dragon cartridge dumps may carry a short leading header, whose length XRoar takes to be `file_size % 256`; the crate applies the same rule. PC-Dragon `.pak` files are snapshots, not cartridges: a RAM dump (optionally run-length compressed) and an info block of registers and display state. The crate is dependency-free and returns typed errors for malformed input.

The PC-Dragon info block is only partly understood. The encoder writes the fields the decoder reads and zeroes the rest, so it is not byte-identical with PC-Dragon's own output.

## Provenance

Graduated from Emu198x's `format-dragon-pak` crate (emu198x/emu198x@63fac19e77), whose history of 7 commits stays in that repository. The name carries the manufacturer, Dragon Data, as well as the machine, as `{manufacturer}-{system}` requires; Emu198x's name had only the machine.

## Licence

GPL-2.0-or-later.
