# format198x-nintendo-game-boy-cartridge

Read **Game Boy and Game Boy Color cartridge headers**, and write their
checksums, in Rust.

Every Game Boy ROM carries a header at `$0100..=$014F`: the title, the Game
Boy Color and Super Game Boy flags, the licensee, the cartridge type (which
memory bank controller, and whether there is RAM, a battery, a clock or a
rumble motor), the ROM and RAM sizes, and two checksums. The boot ROM refuses
a cartridge whose header checksum is wrong; nothing checks the global one.
The crate is dependency-free and returns typed errors for malformed input.

`set_checksums` writes both checksums after a ROM is changed.

```rust
use format198x_nintendo_game_boy_cartridge::{set_checksums, CartridgeHeader, CartridgeType};

let mut rom = vec![0u8; 0x8000];
rom[0x0134..0x0138].copy_from_slice(b"DEMO");
rom[0x0147] = 0x01; // MBC1
let sums = set_checksums(&mut rom)?;

let header = CartridgeHeader::parse(&rom)?;
assert_eq!(header.title, "DEMO");
assert_eq!(header.cart_type, CartridgeType::Mbc1 { ram: false, battery: false });
assert_eq!(header.global_checksum, sums.global);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Every cartridge type Pan Docs documents decodes. Which of those boards an
emulator supports, and how they behave, belongs to the emulator.

Layout: Pan Docs, "The Cartridge Header".

## Provenance

Graduated from Emu198x's `format-nintendo-game-boy-cartridge` crate
(emu198x/emu198x@42257c106), whose history of 53 commits stays in that
repository.

## Licence

GPL-2.0-or-later.
