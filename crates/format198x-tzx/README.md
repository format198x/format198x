# format198x-tzx

Dependency-free decoding and encoding for the block stream shared by ZX
Spectrum `.tzx` and Amstrad CPC `.cdt` tape images.

This crate deliberately stops before playback. TZX pulse lengths use the
format's 3.5 MHz reference clock; converting those lengths into a machine's
timebase belongs to that machine's tape player.

```rust
use format198x_tzx::{Block, Version, Tzx, decode, encode};

let block = Block::standard_speed(1_000, &[0xff, 1, 2, 0xfc])?;
let image = Tzx::new(Version::new(1, 13), vec![block]);
let bytes = encode(&image);
assert_eq!(decode(&bytes)?, image);
# Ok::<(), format198x_tzx::Error>(())
```
