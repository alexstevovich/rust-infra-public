//! Uses Bitcoin's Base58 alphabet (base58btc).

pub use bs58::decode::Error as DecodeError;

/// Encodes bytes using Bitcoin's Base58 alphabet.
pub fn encode(input: &[u8]) -> String {
    bs58::encode(input).into_string()
}

/// Decodes Bitcoin Base58 text into bytes.
pub fn decode(input: &str) -> Result<Vec<u8>, DecodeError> {
    bs58::decode(input).into_vec()
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};

    #[test]
    fn encodes_and_decodes() {
        assert_eq!(encode(b"hello"), "Cn8eVZg");
        assert_eq!(decode("Cn8eVZg").unwrap(), b"hello");
    }

    #[test]
    fn rejects_invalid_input() {
        assert!(decode("0OIl").is_err());
    }
}
