//! Base64 encoding and decoding using the standard alphabet with padding.

use base64::{Engine as _, engine::general_purpose::STANDARD};

pub use base64::DecodeError;

/// Encodes bytes using standard padded Base64.
pub fn encode(input: &[u8]) -> String {
    STANDARD.encode(input)
}

/// Decodes standard padded Base64 into bytes.
pub fn decode(input: &str) -> Result<Vec<u8>, DecodeError> {
    STANDARD.decode(input)
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn encodes_and_decodes() {
        assert_eq!(encode(b"hello"), "aGVsbG8=");
        assert_eq!(decode("aGVsbG8=").unwrap(), b"hello");
    }

    #[test]
    fn rejects_invalid_input() {
        assert!(decode("not base64!").is_err());
    }
}
