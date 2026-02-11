use anyhow::Context;
use object::{File, Object, ObjectSection, ObjectSymbol};
use self_cell::self_cell;
use std::{collections::HashMap, path::{Path, PathBuf}};

self_cell!(
    struct FirmwareElf {
        owner: Vec<u8>,
        #[covariant]
        dependent: File,
    }
);

pub type FirmwareAddressMap = HashMap<u64, String>;

pub struct FirmwareInfo {
    elf: FirmwareElf,
    path : PathBuf,
}

impl FirmwareInfo {
    /// Create a new FirmwareInfo from the given elf file path
    pub fn new(elf_path: &Path) -> anyhow::Result<Self> {
        // Read elf
        let bin_data = std::fs::read(elf_path).context("Could not open elf file")?;
        let elf = FirmwareElf::try_new(bin_data, |data| {
            object::File::parse(&**data).context("Could not parse elf file")
        })?;

        Ok(Self { elf, path: elf_path.to_path_buf() })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Get a mapping of addresses to symbol names for all symbols in the elf file. 
    /// The symbol names are shortened by removing any "::POOL" suffix and anything following it, if present.
    pub fn addr_symbol_map(&self) -> FirmwareAddressMap {
        self.elf.with_dependent(|_, file| {
            file.symbols()
                .filter_map(|symbol| {
                    if let Ok(name) = symbol.name() {
                        Some((symbol.address(), try_extract_short_name(name)))
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    /// Try to get the symbol name for the given address.
    /// Returns None if no symbol with the given address is found or if the symbol name cannot be parsed as utf-8 string
    pub fn get_symbol_name(&self, addr: u64) -> Option<String> {
        self.elf.with_dependent(|_, file| {
            for symbol in file.symbols() {
                if symbol.address() == addr {
                    if let Ok(name) = symbol.name() {
                        return Some(try_extract_short_name(name));
                    }
                }
            }

            None
        })
    }

    /// Try to get the address of the symbol with the given name.
    /// Returns None if no symbol with the given name is found
    pub fn get_symbol_addr(&self, symbol_name: &str) -> Option<u64> {
        self.elf.with_dependent(|_, file| {
            for symbol in file.symbols() {
                if let Ok(name) = symbol.name()
                    && symbol_name == name
                {
                    return Some(symbol.address());
                }
            }

            None
        })
    }

    /// Get a mapping of addresses to symbol names for all symbols in the given section of the elf file.
    /// Returns None if no section with the given name is found. The symbol names are not shortened
    pub fn get_symbols_of_secetion(&self, section_name: &str) -> Option<FirmwareAddressMap> {
        self.elf.with_dependent(|_, file| {
            let Some(section) = file.section_by_name(section_name) else {
                return None;
            };

            // Iterate over symbols and find those in section
            let symbols = file.symbols()
                .filter_map(|symbol| {
                    if symbol.section_index() == Some(section.index())
                    {
                        
                        if let Ok(name) = symbol.name() {
                            Some((symbol.address(), name.to_string()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<FirmwareAddressMap>();

            Some(symbols)
        })
    }

    /// Try to read a null-terminated string from the section data of the symbol with the given name.
    /// Returns None if no symbol with the given name is found, if the symbol name cannot be parsed as utf-8 string, if the section data cannot be read, or if the string cannot be parsed as utf-8 string
    pub fn read_str_from_symbol(&self, symbol_name: &str) -> Option<String> {
        self.elf.with_dependent(|_, file| {
            for symbol in file.symbols() {
                if let Ok(name) = symbol.name()
                    && symbol_name == name
                {
                    // Symbol found, extract data
                    let section = symbol.section_index()?;
                    let section = file.section_by_index(section).ok()?;
                    let data = section.uncompressed_data().ok()?;

                    // Get offset and read string
                    let offset = (symbol.address() - section.address()) as usize;
                    let str_data = &data[offset..];
                    let null_pos = str_data.iter().position(|&b| b == 0)?;

                    // Return string
                    return Some(
                        std::str::from_utf8(&str_data[0..null_pos])
                            .ok()?
                            .to_string(),
                    );
                }
            }
            None
        })
    }

    /// Try to get the address of the _SEGGER_RTT symbol from the firmware
    pub fn get_rtt_symbol_address(&self) -> Option<u64> {
        self.get_symbol_addr("_SEGGER_RTT")
    }
}

/// Helper function to extract short name from full symbol name
fn try_extract_short_name(full_name: &str) -> String {
    let pool_index = full_name.find("::POOL").unwrap_or(full_name.len());
    full_name[0..pool_index].to_string()
}
