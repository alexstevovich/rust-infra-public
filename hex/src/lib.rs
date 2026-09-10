//! Lowercase hexadecimal encoding with case-insensitive decoding.

pub use hex::FromHexError as HexError;

/// Encodes bytes as lowercase hexadecimal text.
pub fn encode(input: &[u8]) -> String {
    hex::encode(input)
}

/// Decodes lowercase or uppercase hexadecimal text into bytes.
pub fn decode(input: &str) -> Result<Vec<u8>, HexError> {
    hex::decode(input)
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn encodes_as_lowercase() {
        assert_eq!(encode(&[0xab, 0xcd, 0xef]), "abcdef");
    }

    #[test]
    fn decodes_both_cases() {
        assert_eq!(decode("68656c6c6f").unwrap(), b"hello");
        assert_eq!(decode("68656C6C6F").unwrap(), b"hello");
    }

    #[test]
    fn rejects_invalid_and_odd_length_input() {
        assert!(decode("xyz").is_err());
        assert!(decode("abc").is_err());
    }
}
