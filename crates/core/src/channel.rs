use thiserror::Error;

pub const LENGTH_PREFIX_SIZE: usize = size_of::<u16>();

/// Prefixes a non-empty message with its big-endian `u16` length.
///
/// # Errors
///
/// Returns an error when the message is empty or larger than `u16::MAX`.
pub fn frame(payload: &[u8]) -> Result<Vec<u8>, ChannelError> {
    if payload.is_empty() {
        return Err(ChannelError::EmptyMessage);
    }

    let length =
        u16::try_from(payload.len()).map_err(|_| ChannelError::MessageTooLarge(payload.len()))?;
    let mut framed = Vec::with_capacity(LENGTH_PREFIX_SIZE + payload.len());
    framed.extend_from_slice(&length.to_be_bytes());
    framed.extend_from_slice(payload);
    Ok(framed)
}

/// Splits an already framed message for transport over a bounded GATT payload.
///
/// # Errors
///
/// Returns an error when `maximum_fragment_size` is zero.
pub fn fragment(framed: &[u8], maximum_fragment_size: usize) -> Result<Vec<&[u8]>, ChannelError> {
    if maximum_fragment_size == 0 {
        return Err(ChannelError::InvalidFragmentSize);
    }

    Ok(framed.chunks(maximum_fragment_size).collect())
}

#[derive(Debug, Default)]
pub struct Decoder {
    buffered: Vec<u8>,
}

impl Decoder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffered: Vec::new(),
        }
    }

    /// Appends one GATT fragment and returns all newly completed messages.
    ///
    /// # Errors
    ///
    /// Returns an error when the stream contains a zero-length message.
    pub fn push(&mut self, fragment: &[u8]) -> Result<Vec<Vec<u8>>, ChannelError> {
        self.buffered.extend_from_slice(fragment);
        let mut messages = Vec::new();
        let mut consumed = 0;

        loop {
            let remaining = &self.buffered[consumed..];
            if remaining.len() < LENGTH_PREFIX_SIZE {
                break;
            }

            let length = usize::from(u16::from_be_bytes([remaining[0], remaining[1]]));
            if length == 0 {
                self.buffered.clear();
                return Err(ChannelError::EmptyMessage);
            }

            let frame_length = LENGTH_PREFIX_SIZE + length;
            if remaining.len() < frame_length {
                break;
            }

            messages.push(remaining[LENGTH_PREFIX_SIZE..frame_length].to_vec());
            consumed += frame_length;
        }

        if consumed > 0 {
            self.buffered.drain(..consumed);
        }

        Ok(messages)
    }

    #[must_use]
    pub fn buffered_len(&self) -> usize {
        self.buffered.len()
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ChannelError {
    #[error("channel messages cannot be empty")]
    EmptyMessage,
    #[error("channel message is {0} bytes; maximum is {maximum}", maximum = u16::MAX)]
    MessageTooLarge(usize),
    #[error("maximum fragment size must be greater than zero")]
    InvalidFragmentSize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_reassembles_minimum_att_fragments() {
        let first = frame(&[0x11; 75]).unwrap();
        let second = frame(&[0x22; 7]).unwrap();
        let stream = [first, second].concat();
        let mut decoder = Decoder::new();
        let mut messages = Vec::new();

        for part in fragment(&stream, 20).unwrap() {
            messages.extend(decoder.push(part).unwrap());
        }

        assert_eq!(messages, vec![vec![0x11; 75], vec![0x22; 7]]);
        assert_eq!(decoder.buffered_len(), 0);
    }

    #[test]
    fn decoder_retains_incomplete_frame() {
        let message = frame(b"hello").unwrap();
        let mut decoder = Decoder::new();

        assert!(decoder.push(&message[..3]).unwrap().is_empty());
        assert_eq!(decoder.buffered_len(), 3);
        assert_eq!(
            decoder.push(&message[3..]).unwrap(),
            vec![b"hello".to_vec()]
        );
    }

    #[test]
    fn empty_wire_message_is_rejected() {
        let mut decoder = Decoder::new();

        assert_eq!(decoder.push(&[0, 0]), Err(ChannelError::EmptyMessage));
        assert_eq!(decoder.buffered_len(), 0);
    }
}
