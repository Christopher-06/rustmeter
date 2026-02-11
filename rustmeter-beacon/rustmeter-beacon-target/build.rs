use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    // Copy linker script to OUT_DIR and add it to linker search path
    let linker_script = fs::read_to_string("rustmeter.x.orig")?;
    let out = &PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out.join("rustmeter.x"), linker_script)?;
    println!("cargo:rustc-link-search={}", out.display());

    Ok(())
}
