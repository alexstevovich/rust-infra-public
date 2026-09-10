//! Secure Nano ID generation.
//!
//! The default is 21 characters drawn from the reference 64-character,
//! URL-safe alphabet: `A-Z`, `a-z`, `0-9`, `_`, and `-`.

use std::{collections::HashSet, error::Error, fmt};

/// The reference Nano ID length.
pub const DEFAULT_LENGTH: usize = 21;

/// The reference Nano ID URL-safe alphabet in its canonical ordering.
pub const DEFAULT_ALPHABET: [char; 64] = [
    '_', '-', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g',
    'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

/// An error produced while validating Nano ID generation parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NanoIdError {
    ZeroLength,
    InvalidAlphabet { distinct_characters: usize },
}

impl fmt::Display for NanoIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLength => write!(formatter, "length must be greater than zero"),
            Self::InvalidAlphabet {
                distinct_characters,
            } => write!(
                formatter,
                "alphabet must contain at least 2 distinct characters (found {distinct_characters})"
            ),
        }
    }
}

impl Error for NanoIdError {}

/// Generates a Nano ID with a custom positive length and alphabet.
pub fn generate(length: usize, alphabet: &[char]) -> Result<String, NanoIdError> {
    if length == 0 {
        return Err(NanoIdError::ZeroLength);
    }

    let distinct_characters = alphabet.iter().copied().collect::<HashSet<_>>().len();
    if distinct_characters < 2 {
        return Err(NanoIdError::InvalidAlphabet {
            distinct_characters,
        });
    }

    Ok(nanoid::nanoid!(length, alphabet))
}

/// Generates a 21-character Nano ID using the reference URL-safe alphabet.
pub fn generate_default() -> String {
    nanoid::nanoid!(DEFAULT_LENGTH, &DEFAULT_ALPHABET)
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_ALPHABET, NanoIdError, generate, generate_default};

    #[test]
    fn generates_default_id() {
        let id = generate_default();
        assert_eq!(id.chars().count(), 21);
        assert!(
            id.chars()
                .all(|character| DEFAULT_ALPHABET.contains(&character))
        );
    }

    #[test]
    fn generates_custom_id() {
        let alphabet = ['A', 'B'];
        let id = generate(10, &alphabet).unwrap();
        assert_eq!(id.len(), 10);
        assert!(id.chars().all(|character| alphabet.contains(&character)));
    }

    #[test]
    fn rejects_invalid_parameters() {
        assert_eq!(generate(0, &['A', 'B']), Err(NanoIdError::ZeroLength));
        assert_eq!(
            generate(10, &['A', 'A']),
            Err(NanoIdError::InvalidAlphabet {
                distinct_characters: 1
            })
        );
    }
}
