use anyhow::Context;
use object::{Object, ObjectSymbol};
use std::{collections::HashMap, path::Path, sync::Arc};

#[derive(Clone)]
pub struct FirmwareAddressMap(Arc<HashMap<u64, String>>);

impl FirmwareAddressMap {
    pub fn new_from_file(file: object::File<'_>) -> Self {
        let mut addr_map: HashMap<u64, String> = HashMap::new();

        for symbol in file.symbols() {
            let addr = symbol.address();
            if addr == 0 {
                continue;
            }

            // Add symbol name if available
            if let Ok(name) = symbol.name() {
                if !name.is_empty() {
                    let demangled = rustc_demangle::demangle(name).to_string();
                    addr_map.insert(addr, demangled);

                    // Reinsert to overwrite potential aliases
                }
            }
        }

        Self(Arc::new(addr_map))
    }

    pub fn new_from_elf_path(elf_path: &Path) -> anyhow::Result<Self> {
        let bin_data = std::fs::read(elf_path).context("Could not open elf file")?;
        let elf_file: object::File<'_> =
            object::File::parse(&*bin_data).context("Could not parse elf file")?;

        Ok(Self::new_from_file(elf_file))
    }

    pub fn get_symbol_name(&self, addr: u64) -> Option<String> {
        self.0.get(&addr).map(try_extract_short_name)
    }
}

/// Helper function to extract short name from full symbol name
fn try_extract_short_name(full_name: &String) -> String {
    let pool_index = full_name.find("::POOL").unwrap_or(full_name.len());
    full_name[0..pool_index].to_string()
}
