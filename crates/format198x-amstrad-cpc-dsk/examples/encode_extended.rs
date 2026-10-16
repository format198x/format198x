//! Convert represented sectors to EDSK; never overwrite an existing path.
use std::{env, error::Error, fs, fs::OpenOptions, io::Write};

use format198x_amstrad_cpc_dsk::{encode_extended, parse};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let input = args
        .next()
        .ok_or("usage: encode_extended INPUT NEW_OUTPUT")?;
    let output = args
        .next()
        .ok_or("usage: encode_extended INPUT NEW_OUTPUT")?;
    if args.next().is_some() {
        return Err("usage: encode_extended INPUT NEW_OUTPUT".into());
    }
    let encoded = encode_extended(&parse(&fs::read(input)?)?)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&encoded)?;
    Ok(())
}
