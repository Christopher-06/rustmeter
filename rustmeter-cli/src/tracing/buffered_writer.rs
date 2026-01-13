use std::{mem, path::PathBuf};

use anyhow::{Context, Result};
use polars::prelude::*;

pub trait WritableBuffer {
    type ItemType;

    /// Create a new empty buffer for given stream ID
    fn new(stream_id: u32) -> Self;
    /// Push a item into the buffer
    fn push(&mut self, item: &Self::ItemType) -> Result<()>;
    /// Check if buffer is full and needs flushing
    fn is_full(&self) -> bool;
    /// Get current length of the buffer
    fn len(&self) -> usize;
    /// Convert buffer into a DataFrame
    fn as_dataframe(self) -> Result<DataFrame>;
}

pub struct BufferedWriter<T: WritableBuffer> {
    buffer: T,

    /// Folder path for target trace files
    path: PathBuf,
    file_prefix: String,
    /// Current file index
    index: usize,
    stream_id: u32,
}

impl<T: WritableBuffer> BufferedWriter<T> {
    pub fn new(path: PathBuf, file_prefix: String, stream_id: u32) -> Result<Self> {
        Ok(Self {
            path,
            file_prefix,
            index: 0,
            stream_id,
            buffer: T::new(stream_id),
        })
    }

    /// Feed a tracing item into the buffer, flushes if necessary
    pub fn feed(&mut self, item: &T::ItemType) -> Result<()> {
        self.buffer.push(item)?;

        // Write if buffer is full
        if self.buffer.is_full() {
            self.flush()?;
        }

        Ok(())
    }

    /// Flush current buffer to file
    pub fn flush(&mut self) -> Result<()> {
        if self.buffer.len() > 0 {
            // Create file
            let filepath = self.path.join(format!(
                "{}_s{:03}_i{:06}.parquet",
                self.file_prefix, self.stream_id, self.index
            ));
            let mut file = std::fs::File::create(&filepath)
                .context(format!("Failed to create trace file (#{})", self.index + 1))?;

            // Swap buffer with new empty one
            let mut buffer = T::new(self.stream_id);
            mem::swap(&mut self.buffer, &mut buffer);
            self.index += 1;

            // Write buffer as parquet
            let mut df = buffer.as_dataframe()?;
            ParquetWriter::new(&mut file)
                .with_compression(ParquetCompression::Snappy)
                .with_statistics(StatisticsOptions::default())
                .finish(&mut df)
                .context("Failed to write buffer to parquet file!")?;
        }

        Ok(())
    }
}

impl<T: WritableBuffer> Drop for BufferedWriter<T> {
    fn drop(&mut self) {
        // Flush remaining data
        if let Err(e) = self.flush() {
            eprintln!("[Error] Failed to flush remaining data: {e}");
        }
    }
}
