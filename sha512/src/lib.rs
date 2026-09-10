//! Pure SHA-512 hashing helpers.

use sha2::{Digest, Sha512};
use std::fmt::Write;

/// Computes the SHA-512 digest of `input`.
pub fn hash(input: &[u8]) -> [u8; 64] {
    Sha512::digest(input).into()
}

/// Formats a SHA-512 digest as canonical lowercase hexadecimal.
pub fn to_hex(digest: &[u8; 64]) -> String {
    let mut output = String::with_capacity(128);
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
            concat!(
                "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce",
                "47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
            )
        );
    }

    #[test]
    fn hashes_abc() {
        assert_eq!(
            to_hex(&hash(b"abc")),
            concat!(
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a",
                "2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
            )
        );
    }
}
