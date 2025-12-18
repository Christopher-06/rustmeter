use std::path::PathBuf;

use crossbeam::channel::{Receiver, Sender};
use defmt_decoder::Table;

use crate::logs::defmt_line::DefmtLine;

pub struct DefmtDecoding {
    defmt_logs_recver: Receiver<DefmtLine>,
}

impl DefmtDecoding {
    pub fn new(
        elf_path: &PathBuf,
        defmt_bytes_recver: Receiver<Box<[u8]>>,
        print_to_console: bool,
    ) -> anyhow::Result<Self> {
        let table = read_defmt_table(elf_path)?;

        let (defmt_logs_sender, defmt_logs_recver) = crossbeam::channel::unbounded();
        std::thread::spawn(move || {
            defmt_decoder_thread(
                table,
                defmt_bytes_recver,
                defmt_logs_sender,
                print_to_console,
            );
        });

        Ok(Self { defmt_logs_recver })
    }

    pub fn get_defmt_logs_recver(&self) -> Receiver<DefmtLine> {
        self.defmt_logs_recver.clone()
    }
}

fn defmt_decoder_thread(
    table: defmt_decoder::Table,
    defmt_bytes_recver: Receiver<Box<[u8]>>,
    defmt_logs_sender: Sender<DefmtLine>,
    print_to_console: bool,
) {
    let mut decoder = table.new_stream_decoder();

    loop {
        // receive defmt bytes
        match defmt_bytes_recver.recv() {
            Ok(defmt_bytes) => {
                decoder.received(&defmt_bytes);
            }
            Err(_) => break, // channel closed
        };

        // decode defmt messages
        loop {
            match decoder.decode() {
                Ok(frame) => {
                    let defmt_line = frame.try_into(); // turn into DefmtLine
                    match defmt_line {
                        Err(e) => {
                            println!("[DEFMT Line Error] {}", e);
                        }
                        Ok(defmt_line) => {
                            if print_to_console {
                                println!("{defmt_line}");
                            }

                            if defmt_logs_sender.send(defmt_line).is_err() {
                                return; // channel closed ==> exit thread
                            }
                        }
                    };
                }
                Err(defmt_decoder::DecodeError::UnexpectedEof) => {
                    // More data needed, break the inner loop
                    break;
                }
                Err(e) => {
                    eprintln!("[DEFMT Error] {}", e);
                    break;
                }
            }
        }
    }
}

fn read_defmt_table(elf_path: &PathBuf) -> anyhow::Result<defmt_decoder::Table> {
    // read elf file
    let bytes = std::fs::read(elf_path)
        .map_err(|e| anyhow::anyhow!("Failed to read elf file {:?}: {}", elf_path, e))?;

    // parse defmt table
    let table = Table::parse(&bytes)
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse defmt table from elf file {:?}: {}",
                elf_path,
                e
            )
        })?
        .ok_or_else(|| anyhow::anyhow!("No .defmt data found in elf file {:?}", elf_path))?;

    // Check if all indices have location info
    let locs = table.get_locations(&bytes)?;
    let all_locs = table.indices().all(|idx| locs.contains_key(&(idx as u64)));
    if !all_locs {
        println!("(BUG) location info is incomplete; it will be omitted from the output");
    }

    Ok(table)
}
