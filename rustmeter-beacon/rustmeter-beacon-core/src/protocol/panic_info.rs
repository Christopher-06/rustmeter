#![allow(unused_imports)]
use crate::{buffer::BufferWriter, tracing::ReadTracingError};
use core::panic::{Location, PanicInfo};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct CustomPanicInfo {
    core_id: u8,
    panic_sys_time: u64,
    // Message Info
    msg_length: u16,
    #[cfg(not(feature = "std"))]
    message: Option<*const u8>,
    #[cfg(feature = "std")]
    message: Option<String>,
    // Location Info
    line: u16,
    fname_length: u16,
    #[cfg(not(feature = "std"))]
    filename: Option<*const u8>,
    #[cfg(feature = "std")]
    filename: Option<String>,
}

impl CustomPanicInfo {
    /// Create a CustomPanicInfo from a core::panic::PanicInfo
    #[cfg(not(feature = "std"))]
    pub fn from_panic(panic_info: &PanicInfo<'_>, core_id: u8, panic_sys_time : u64) -> Self {
        let (message, msg_length) = match panic_info.message().as_str() {
            Some(msg) => (Some(msg.as_ptr()), msg.len() as u16),
            None => (None, 0),
        };

        let location = panic_info.location();
        let line = location.map_or(0, Location::line) as u16;

        let (filename, fname_length) = match location {
            Some(loc) => {
                let fname = loc.file();
                (Some(fname.as_ptr()), fname.len() as u16)
            }
            None => (None, 0),
        };

        CustomPanicInfo {
            core_id,
            panic_sys_time,
            msg_length,
            message,
            line,
            fname_length,
            filename,
        }
    }

    /// Write the CustomPanicInfo to a BufferWriter
    #[cfg(not(feature = "std"))]
    pub fn write_bytes<T : BufferWriter>(&self, writer: &mut T) {
        writer.write_byte(self.core_id);
        writer.write_u64(self.panic_sys_time);
        
        // Write message
        writer.write_u16(self.msg_length);
        if let Some(msg) = self.message {
            writer
                .write_bytes(unsafe { core::slice::from_raw_parts(msg, self.msg_length as usize) });
        }

        // Write location info
        writer.write_u32(self.line as u32);
        writer.write_u16(self.fname_length);
        if let Some(fname) = self.filename {
            writer.write_bytes(unsafe {
                core::slice::from_raw_parts(fname, self.fname_length as usize)
            });
        }
    }

    /// Read a CustomPanicInfo from a BufferReader
    #[cfg(feature = "std")]
    pub fn read_bytes(
        reader: &mut crate::buffer::BufferReader,
    ) -> Result<Self, crate::tracing::ReadTracingError> {
        let core_id = reader.read_byte()?;
        let panic_sys_time = reader.read_u64()?;

        // Read message
        let msg_length = reader.read_u16()?;
        let message = if msg_length > 0 {
            let msg_bytes = reader.read_bytes(msg_length as usize)?;
            Some(
                str::from_utf8(msg_bytes)
                    .map_err(ReadTracingError::StringConversionError)?
                    .into(),
            )
        } else {
            None
        };

        // Read location info
        let line = reader.read_u32()? as u16;
        let fname_length = reader.read_u16()?;
        let filename = if fname_length > 0 {
            let fname_bytes = reader.read_bytes(fname_length as usize)?;
            Some(
                str::from_utf8(fname_bytes)
                    .map_err(ReadTracingError::StringConversionError)?
                    .into(),
            )
        } else {
            None
        };

        Ok(CustomPanicInfo {
            core_id,
            panic_sys_time,
            msg_length,
            message,
            line,
            fname_length,
            filename,
        })
    }
}

#[cfg(feature = "std")]
impl CustomPanicInfo {
    pub fn core_id(&self) -> u8 {
        self.core_id
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn line(&self) -> u16 {
        self.line
    }

    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    pub fn sys_time(&self) -> u64 {
        self.panic_sys_time
    }
}

#[cfg(feature = "std")]
impl std::fmt::Display for CustomPanicInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Panic on core {}: '{}' at {}:{}",
            self.core_id(),
            self.message().unwrap_or_default(),
            self.filename().unwrap_or_default(),
            self.line()
        )
    }
}
