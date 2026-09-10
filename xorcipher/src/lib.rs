//! Repeating-key XOR byte transformation.
//!
//! [`apply`] serves as both encode and decode because XOR is self-inverse:
//! applying it twice with the same key restores the original input.
//!
//! This is not cryptographically secure. Repeating-key XOR is vulnerable to
//! frequency analysis, especially when the key is short relative to the input.
//! This crate is a reversible byte-transformation utility, not a security tool.

/// Applies a repeating XOR key to input bytes.
///
/// An empty key acts as the identity transformation.
pub fn apply(key: &[u8], input: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return input.to_vec();
    }

    input
        .iter()
        .zip(key.iter().cycle())
        .map(|(input, key)| input ^ key)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn applying_twice_restores_input() {
        let input = b"TIFA LOCKHART";
        let encoded = apply(b"cloud", input);
        assert_eq!(apply(b"cloud", &encoded), input);
    }

    #[test]
    fn empty_key_is_identity() {
        assert_eq!(apply(b"", b"hello"), b"hello");
    }
}
