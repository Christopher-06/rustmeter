use crate::buffer::BufferWriter;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Request {
    GetGlobalClockDefinition,
    GetCoreClockReference { core_id: u8 },
}

impl Request {
    pub fn type_id(&self) -> u8 {
        match self {
            Request::GetGlobalClockDefinition => 1,
            Request::GetCoreClockReference { .. } => 2,
        }
    }

    /// Serialize the Request into bytes
    pub fn write_bytes<T: BufferWriter>(&self, writer: &mut T) {
        writer.write_byte(self.type_id());

        match self {
            Request::GetGlobalClockDefinition => {}
            Request::GetCoreClockReference { core_id } => {
                writer.write_byte(*core_id);
            }
        }
    }

    /// Deserialize a Request from bytes
    pub fn from_bytes<'a, T: Iterator<Item = &'a u8>>(mut buffer: T) -> Option<(Self, usize)> {
        let req_id = buffer.next()?;

        match req_id {
            1 => Some((Request::GetGlobalClockDefinition, 1)),
            2 => {
                let core_id = *buffer.next()?;
                Some((Request::GetCoreClockReference { core_id }, 2))
            }
            _ => None,
        }
    }
}
