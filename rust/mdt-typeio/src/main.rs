use std::{env, error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = env::args().nth(1).ok_or("usage: mdt-typeio <output-dir>")?;
    let output_dir = Path::new(&output_dir);
    fs::create_dir_all(output_dir)?;

    let text = mdt_typeio::generate_typeio_goldens();
    fs::write(output_dir.join("typeio-goldens.txt"), text)?;
    Ok(())
}
