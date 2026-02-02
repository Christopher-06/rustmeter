use std::{u8, usize};

use crate::{
    espflash::{
        SerialFrameError,
        framing::{FrameMode, FrameStream, SerialFrame},
    },
    tracing::{CoreTracingData, TracingDecodeError},
};

pub type OptionalResult<T, E> = Result<Option<T>, E>;
pub type DecodingResult = OptionalResult<CoreTracingData, TracingDecodeError>;

/// A serial data decoder that decodes incoming byte streams into frames
pub struct SerialDecoder {
    buffer: Vec<u8>,
    valid_in_stream: bool,
    prev_normal_payload: Option<Box<[u8]>>,
    next_normal_seq_ids: [Option<u8>; 2],
    next_panic_seq_ids: [Option<u8>; 2],
}

impl SerialDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            valid_in_stream: false,
            prev_normal_payload: None,
            next_normal_seq_ids: [None, None],
            next_panic_seq_ids: [None, None],
        }
    }

    /// Feed new data into the decoder
    pub fn feed(&mut self, buf: &[u8]) {
        self.buffer.extend_from_slice(buf);
    }

    /// Try to decode next frame from the internal buffer and return how many bytes should be drained
    fn decoding(&mut self) -> (DecodingResult, usize) {
        // Get next complete frames from buffer
        let next_normal = get_next_complete_frame(&self.buffer, Some(FrameMode::Normal));
        let next_normal_pos = next_normal
            .as_ref()
            .map(|(pos, _)| *pos)
            .unwrap_or(usize::MAX);
        let next_normal_len = next_normal
            .as_ref()
            .map(|(_, frame)| frame.len())
            .flatten()
            .unwrap_or(0);
        let next_panic = get_next_complete_frame(&self.buffer, Some(FrameMode::Panic));
        let next_panic_pos = next_panic
            .as_ref()
            .map(|(pos, _)| *pos)
            .unwrap_or(usize::MAX);
        let next_panic_len = next_panic
            .as_ref()
            .map(|(_, frame)| frame.len())
            .flatten()
            .unwrap_or(0);

        // Validate frames
        let mut next_normal_seq_ids = self.next_normal_seq_ids.clone();
        let next_normal = next_normal.map(|(pos, frame)| {
            validate_frame_updated(frame, &mut next_normal_seq_ids).map(|f| (pos, f))
        });
        let mut next_panic_seq_ids = self.next_panic_seq_ids.clone();
        let next_panic = next_panic.map(|(pos, frame)| {
            validate_frame_updated(frame, &mut next_panic_seq_ids).map(|f| (pos, f))
        });

        // Select frame to process
        let (pos, frame) = match (next_normal, next_panic) {
            (None, None) => return (Ok(None), 0), // no complete frames yet
            (Some(Ok((n_pos, n_frame))), _) => (n_pos, n_frame), // prioritize normal frame
            (None, Some(Ok((p_pos, p_frame)))) => (p_pos, p_frame), // only panic frame
            (None, Some(Err(e))) => {
                // nothing normal and panic frame invalid
                return (
                    Err(TracingDecodeError::SerialFrameError(e)),
                    (next_panic_pos + next_panic_len),
                );
            }
            (Some(Err(_)), Some(Err(p_err))) => {
                // both invalid, return normal error
                let min = usize::min(
                    next_normal_pos + next_normal_len,
                    next_panic_pos + next_panic_len,
                );
                return (Err(TracingDecodeError::SerialFrameError(p_err)), min);
            }
            (Some(Err(n_err)), Some(Ok((p_pos, p_frame)))) => {
                // on checksum error of normal frame, prioritize panic frame (panic disrupted normal message)
                if let SerialFrameError::ChecksumMismatch = n_err {
                    (p_pos, p_frame)
                } else {
                    // otherwise return normal frame error
                    return (
                        Err(TracingDecodeError::SerialFrameError(n_err)),
                        (next_normal_pos + next_normal_len),
                    );
                }
            }
            (Some(Err(n_err)), None) => {
                // Check if normal frame got disrupted because of starting panic mode, but panic frame incomplete (because None)
                if let SerialFrameError::ChecksumMismatch = n_err {
                    // look for 0xFE in buffer after normal frame start and before next normal frame ends
                    let slice =
                        &self.buffer[(next_normal_pos + 1)..(next_normal_pos + next_normal_len)];
                    if slice.contains(&(FrameMode::Panic as u8)) {
                        // wait for completion (Will then get handled by ((Some(Err(n_err)), Some(Ok((p_pos, p_frame)))) branch)
                        return (Ok(None), 0);
                    }
                }
                // otherwise return normal frame error
                return (
                    Err(TracingDecodeError::SerialFrameError(n_err)),
                    (next_normal_pos + next_normal_len),
                );
            }
        };

        let mode = frame.mode().unwrap().unwrap();

        // Check for duplicate frames
        let duplicate = self
            .prev_normal_payload
            .as_ref()
            .map_or(false, |prev| frame.data().unwrap() == prev.as_ref());
        if duplicate {
            // return duplicate error in normal mode (normal to panic transition is allowed to duplicate last normal frame)
            if mode == FrameMode::Normal {
                return (
                    Err(TracingDecodeError::SerialFrameError(
                        SerialFrameError::DuplicateData,
                    )),
                    (pos + frame.len().unwrap()),
                );
            } else {
                // ignore in panic mode
                return (Ok(None), (pos + frame.len().unwrap()));
            }
        }

        // Update
        match mode {
            FrameMode::Normal => {
                self.prev_normal_payload = Some(frame.data_as_boxed().unwrap());
                self.next_normal_seq_ids = next_normal_seq_ids;
            }
            FrameMode::Panic => {
                self.next_panic_seq_ids = next_panic_seq_ids;
            }
        }

        // Convert to tracing data
        let core_id = frame.core_id().unwrap();
        let payload = frame.data_as_boxed().unwrap();
        let tracing_data = match core_id {
            0 => CoreTracingData::Core0(payload.clone()),
            1 => CoreTracingData::Core1(payload.clone()),
            // This should never happen because of one bit core id.
            _ => unreachable!("Invalid core id parsed."),
        };

        let len = frame.len().unwrap();
        (Ok(Some(tracing_data)), pos + len)
    }

    /// Try to decode next frame from the internal buffer
    pub fn try_decode(&mut self) -> DecodingResult {
        let (result, to_drain) = self.decoding();
        self.buffer.drain(0..to_drain);

        if result.is_ok() {
            self.valid_in_stream = true;
            result
        } else {
            if self.valid_in_stream {
                self.valid_in_stream = false;
                self.next_normal_seq_ids = [None, None];
                self.next_panic_seq_ids = [None, None];
                self.prev_normal_payload = None;
                result
            } else {
                // discard error if no valid frames have been seen yet
                Ok(None)
            }
        }
    }
}

fn validate_frame_updated<'a>(
    frame: SerialFrame<'a>,
    seq_ids: &mut [Option<u8>; 2],
) -> Result<SerialFrame<'a>, SerialFrameError> {
    // Checksum
    if !frame.verify_checksum().unwrap_or(false) {
        return Err(SerialFrameError::ChecksumMismatch);
    }

    // Check Mode (should never fail)
    let _mode = frame.mode()?;

    // Seq id
    let core_id = frame.core_id().unwrap() as usize;
    let seq_id = frame.seq_id().unwrap();
    let expected = seq_ids[core_id].unwrap_or(seq_id);
    if seq_id != expected {
        return Err(SerialFrameError::SequenceIdMismatch {
            expected,
            received: seq_id,
            core_id: core_id as u8,
        });
    }
    seq_ids[core_id] = Some((seq_id + 1) % 128);

    Ok(frame)
}

/// Get next complete frame from the buffer. Returns None if no complete frame is found with
/// the given mode. Does not check for validity.
fn get_next_complete_frame(buf: &[u8], mode: Option<FrameMode>) -> Option<(usize, SerialFrame)> {
    // Get next frame from buffer
    let stream = FrameStream::from_bytes(buf);
    let (frame_pos, frame) = match stream.get_first_frame_start(mode) {
        Some((pos, frame)) => (pos, frame),
        None => return None,
    };

    // Check for completion
    if !frame.is_complete() {
        None
    } else {
        Some((frame_pos, frame))
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;

    use super::*;
    use crate::espflash::framing::{FrameMode, calculate_checksum};

    macro_rules! feed {
        ($dec:expr, $m:expr, $bytes:expr) => {
            // Checksum via: Mode + $bytes
            let mut checksum_data = vec![$m as u8];
            checksum_data.extend_from_slice($bytes);
            let checksum = calculate_checksum(&checksum_data);

            feed!($dec, $m, $bytes, checksum);
        };
        ($dec:expr, $m:expr, $bytes:expr, $checksum:expr) => {
            $dec.feed(&[$m as u8]);
            $dec.feed($bytes);
            $dec.feed(&[$checksum]);
        };
    }

    /// Test disrupted transition from Normal to Panic Mode
    #[test]
    fn test_normal_to_panic_disrupted() {
        let mut decoder = SerialDecoder::new();

        // feed first complete normal frame (Core0, seq id 0)
        feed!(decoder, FrameMode::Normal, &[0x00, 0x03, 0x10, 0x20, 0x30]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0x10, 0x20, 0x30]))))
        );

        // feed second complete normal frame (Core0, seq id 1)
        feed!(decoder, FrameMode::Normal, &[0x01, 0x02, 0xAA, 0xBB]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0xAA, 0xBB]))))
        );
        assert_eq!(decoder.try_decode(), Ok(None)); // no more frames

        // feed disrupted normal frame (Core0, seq id 2)
        feed!(decoder, FrameMode::Normal, &[0x02, 0x05, 0xDE, 0xAD]); // incomplete
        assert_eq!(decoder.try_decode(), Ok(None)); // no frame yet

        // feed panic frame (Core1, seq id 0)
        feed!(decoder, FrameMode::Panic, &[0x80, 0x03, 0xAB, 0xBC, 0xCD]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core1(Box::new([0xAB, 0xBC, 0xCD])))) // retrieve panic frame
        );

        // feed next panic frame (Core1, seq id 1)
        feed!(decoder, FrameMode::Panic, &[0x81, 0x01, 0xDE]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core1(Box::new([0xDE]))))
        );

        // feed next panic frame (Core0, seq id 1 (because of duplicate))
        feed!(decoder, FrameMode::Panic, &[0x01, 0x02, 0xFE, 0xED]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0xFE, 0xED]))))
        );
        assert_eq!(decoder.try_decode(), Ok(None)); // no more frames
    }

    /// Test clean transition from Normal to Panic Mode (Complete last Normal message with and without duplicate)
    #[test]
    fn test_normal_to_panic_clean() {
        for with_dup in [false, true] {
            let mut decoder = SerialDecoder::new();

            // feed first complete normal frame (Core0, seq id 0)
            feed!(decoder, FrameMode::Normal, &[0x00, 0x03, 0x10, 0x20, 0x30]);
            assert_eq!(
                decoder.try_decode(),
                Ok(Some(CoreTracingData::Core0(Box::new([0x10, 0x20, 0x30]))))
            );

            // feed second complete normal frame (Core0, seq id 1)
            feed!(decoder, FrameMode::Normal, &[0x01, 0x02, 0xAA, 0xBB]);
            assert_eq!(
                decoder.try_decode(),
                Ok(Some(CoreTracingData::Core0(Box::new([0xAA, 0xBB]))))
            );
            assert_eq!(decoder.try_decode(), Ok(None)); // no more frames

            // feed duplicate last normal frame
            if with_dup {
                feed!(decoder, FrameMode::Panic, &[0x00, 0x02, 0xAA, 0xBB]);
                assert_eq!(decoder.try_decode(), Ok(None)); // duplicate ignored
            }

            // feed panic frame (Core1, seq id 0)
            feed!(decoder, FrameMode::Panic, &[0x80, 0x03, 0xAB, 0xBC, 0xCD]);
            assert_eq!(
                decoder.try_decode(),
                Ok(Some(CoreTracingData::Core1(Box::new([0xAB, 0xBC, 0xCD])))) // retrieve panic frame
            );

            // feed next panic frame (Core1, seq id 1)
            feed!(decoder, FrameMode::Panic, &[0x81, 0x01, 0xDE]);
            assert_eq!(
                decoder.try_decode(),
                Ok(Some(CoreTracingData::Core1(Box::new([0xDE]))))
            );

            // feed next panic frame (Core0, seq id 1 (because of duplicate))
            feed!(decoder, FrameMode::Panic, &[0x01, 0x02, 0xFE, 0xED]);
            assert_eq!(
                decoder.try_decode(),
                Ok(Some(CoreTracingData::Core0(Box::new([0xFE, 0xED]))))
            );
            assert_eq!(decoder.try_decode(), Ok(None)); // no more frames
        }
    }

    #[test]
    fn test_new_to_normal() {
        let mut decoder = SerialDecoder::new();

        // feed incomplete frame
        decoder.feed(&[0xFF, 0x00, 0x05, 0x01, 0x02]);
        assert_eq!(decoder.try_decode(), Ok(None));
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed rest of frame with invalid checksum
        // should discard invalid frame
        decoder.feed(&[0x03, 0x04, 0x05, 0x00]);
        assert_eq!(
            decoder.try_decode(),
            Err(SerialFrameError::ChecksumMismatch.into())
        );
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed complete valid frame
        feed!(decoder, FrameMode::Normal, &[0x00, 0x03, 0x10, 0x20, 0x30]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0x10, 0x20, 0x30]))))
        );
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed next valid frame
        feed!(decoder, FrameMode::Normal, &[0x01, 0x02, 0xAA, 0xBB]);
        let result = decoder.try_decode();
        assert_eq!(
            result,
            Ok(Some(CoreTracingData::Core0(Box::new([0xAA, 0xBB]))))
        );
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed next valid frame (Core1)
        feed!(decoder, FrameMode::Normal, &[0x80, 0x03, 0xAB, 0xBC, 0xCD]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core1(Box::new([0xAB, 0xBC, 0xCD]))))
        );
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed last valid frame (Core0)
        feed!(decoder, FrameMode::Normal, &[0x02, 0x01, 0xDE]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0xDE]))))
        );
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed last valid frame (Core0) with wrong seq id
        feed!(decoder, FrameMode::Normal, &[0x05, 0x01, 0xDE]);
        assert_eq!(
            decoder.try_decode(),
            Err(SerialFrameError::SequenceIdMismatch {
                expected: 3,
                received: 5,
                core_id: 0
            }
            .into())
        );
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed last valid frame (Core0) with any correct seq id
        feed!(decoder, FrameMode::Normal, &[0x09, 0x01, 0xDE]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0xDE]))))
        );
        assert_eq!(decoder.try_decode(), Ok(None));
    }

    #[test]
    fn test_new_to_panic() {
        let mut decoder = SerialDecoder::new();

        // feed incomplete frame
        decoder.feed(&[0xFE, 0x00, 0x05, 0x01, 0x02]);
        assert_eq!(decoder.try_decode(), Ok(None));
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed rest of frame with invalid checksum
        // should discard invalid frame
        decoder.feed(&[0x03, 0x04, 0x05, 0x00]);
        assert_eq!(
            decoder.try_decode(),
            Err(SerialFrameError::ChecksumMismatch.into())
        );
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed complete valid frame
        feed!(decoder, FrameMode::Panic, &[0x00, 0x03, 0x10, 0x20, 0x30]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0x10, 0x20, 0x30]))))
        );

        // feed next valid frame
        feed!(decoder, FrameMode::Panic, &[0x01, 0x02, 0xAA, 0xBB]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0xAA, 0xBB]))))
        );

        // feed next valid frame (Core1)
        feed!(decoder, FrameMode::Panic, &[0x80, 0x03, 0xAB, 0xBC, 0xCD]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core1(Box::new([0xAB, 0xBC, 0xCD]))))
        );

        // feed last valid frame (Core0) with wrong seq id
        feed!(decoder, FrameMode::Panic, &[0x05, 0x01, 0xDE]);
        assert_eq!(
            decoder.try_decode(),
            Err(SerialFrameError::SequenceIdMismatch {
                expected: 2,
                received: 5,
                core_id: 0
            }
            .into())
        );
        assert_eq!(decoder.try_decode(), Ok(None));

        // feed last valid frame (Core0) with any correct seq id
        feed!(decoder, FrameMode::Panic, &[0x09, 0x01, 0xDE]);
        assert_eq!(
            decoder.try_decode(),
            Ok(Some(CoreTracingData::Core0(Box::new([0xDE]))))
        );
        assert_eq!(decoder.try_decode(), Ok(None));
    }
}
