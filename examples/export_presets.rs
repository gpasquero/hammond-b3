//! Write the built-in factory presets to `presets/*.toml`.
//!
//! Run with: `cargo run --example export_presets --no-default-features`

use hammond_b3::preset::factory_presets;
use std::fs;
use std::path::Path;

fn main() -> std::io::Result<()> {
    let dir = Path::new("presets");
    fs::create_dir_all(dir)?;
    for preset in factory_presets() {
        let slug = preset
            .name
            .to_lowercase()
            .replace(' ', "-")
            .replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "");
        let path = dir.join(format!("{slug}.toml"));
        let toml = preset.to_toml().expect("serialize preset");
        fs::write(&path, toml)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
