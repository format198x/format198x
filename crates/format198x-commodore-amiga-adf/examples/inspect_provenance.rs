use format198x_commodore_amiga_adf::{Disk, FileSystem, Volume};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut volume = Volume::new("Workbench", FileSystem::Ofs);
    volume.add_file("S/Startup-Sequence", b"echo hello\n")?;
    let image = volume.build()?;

    let provenance = Disk::open(&image)?.inspect("S/Startup-Sequence")?;
    for component in &provenance.components {
        println!(
            "{}: header block {}",
            component.name, component.header_block
        );
    }
    if let Some(file) = provenance.file {
        println!("pointer-table order: {:?}", file.pointer_table_data);
        println!("OFS linked chain: {:?}", file.ofs_data_chain);
    }

    Ok(())
}
