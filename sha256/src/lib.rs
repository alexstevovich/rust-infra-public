//! Pure SHA-256 hashing helpers.

use sha2::{Digest, Sha256};
use std::fmt::Write;

/// Computes the SHA-256 digest of `input`.
pub fn hash(input: &[u8]) -> [u8; 32] {
    Sha256::digest(input).into()
}

/// Formats a SHA-256 digest as canonical lowercase hexadecimal.
pub fn to_hex(digest: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{hash, to_hex};

    #[test]
    fn hashes_empty_input() {
        assert_eq!(
            to_hex(&hash(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hashes_abc() {
        assert_eq!(
            to_hex(&hash(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
