use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// {"reason":"compiler-artifact","package_id":"registry+https://github.com/rust-lang/crates.io-index#esp-rtos@0.2.0","manifest_path":"C:\\Users\\chris\\.cargo\\registry\\src\\index.crates.io-1949cf8c6b5b557f\\esp-rtos-0.2.0\\Cargo.toml","target":{"kind":["lib"],"crate_types":["lib"],"name":"esp_rtos","src_path":"C:\\Users\\chris\\.cargo\\registry\\src\\index.crates.io-1949cf8c6b5b557f\\esp-rtos-0.2.0\\src\\lib.rs","edition":"2024","doc":true,"doctest":true,"test":true},"profile":{"opt_level":"s","debuginfo":2,"debug_assertions":true,"overflow_checks":true,"test":false},"features":["default","defmt","embassy","esp32"],"filenames":["C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\target\\xtensa-esp32-none-elf\\debug\\deps\\libesp_rtos-0c0bbec6a5ad5299.rlib","C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\target\\xtensa-esp32-none-elf\\debug\\deps\\libesp_rtos-0c0bbec6a5ad5299.rmeta"],"executable":null,"fresh":true}
// {"reason":"compiler-artifact","package_id":"path+file:///C:/Users/chris/Documents/Projekte/embassy-tracer/esp32-embassy-tracer#0.1.0","manifest_path":"C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\Cargo.toml","target":{"kind":["lib"],"crate_types":["lib"],"name":"esp32_embassy_tracer","src_path":"C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\src\\lib.rs","edition":"2024","doc":true,"doctest":true,"test":true},"profile":{"opt_level":"s","debuginfo":2,"debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\target\\xtensa-esp32-none-elf\\debug\\libesp32_embassy_tracer.rlib","C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\target\\xtensa-esp32-none-elf\\debug\\deps\\libesp32_embassy_tracer-cc08d6f9ed74997c.rmeta"],"executable":null,"fresh":true}
// {"reason":"compiler-artifact","package_id":"path+file:///C:/Users/chris/Documents/Projekte/embassy-tracer/esp32-embassy-tracer#0.1.0","manifest_path":"C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\Cargo.toml","target":{"kind":["bin"],"crate_types":["bin"],"name":"esp32-embassy-tracer","src_path":"C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\src\\bin\\main.rs","edition":"2024","doc":true,"doctest":false,"test":true},"profile":{"opt_level":"s","debuginfo":2,"debug_assertions":true,"overflow_checks":true,"test":false},"features":[],"filenames":["C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\target\\xtensa-esp32-none-elf\\debug\\esp32-embassy-tracer"],"executable":"C:\\Users\\chris\\Documents\\Projekte\\embassy-tracer\\esp32-embassy-tracer\\target\\xtensa-esp32-none-elf\\debug\\esp32-embassy-tracer","fresh":true}
// {"reason":"build-finished","success":true}
// map to this enum:

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "reason")]
pub enum CargoBuildMessage {
    #[serde(rename = "compiler-artifact")]
    CompilerArtifact {
        package_id: String,
        executable: Option<String>,
    },
    #[serde(rename = "build-finished")]
    BuildFinished { success: bool },
    // Add other message types as needed to not print them
    #[serde(rename = "build-script-executed")]
    BuildScriptExecuted,
}

impl CargoBuildMessage {
    pub fn from_build_line(line: &str) -> anyhow::Result<CargoBuildMessage> {
        // Try parse
        match serde_json::from_str::<CargoBuildMessage>(line) {
            Ok(message) => Ok(message),
            Err(e) => Err(anyhow::anyhow!("Failed to parse cargo build message: {e}")),
        }
    }

    /// Returns the executable path if this is a CompilerArtifact message with an executable info
    pub fn get_elf_path(&self) -> Option<PathBuf> {
        if let CargoBuildMessage::CompilerArtifact { executable, .. } = self {
            if let Some(path) = executable {
                return Some(PathBuf::from(path));
            }
        }

        None
    }
}
