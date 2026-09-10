//! Arbitrary-alphabet positional radix conversion.
//!
//! The effective symbol set is derived from the supplied alphabet by retaining
//! characters in first-occurrence order and discarding later duplicates. For
//! example, `"ABABABA"` becomes `"AB"`, giving a base-2 alphabet.

use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};
use std::{collections::HashSet, error::Error, fmt};

/// An error produced while validating an alphabet or decoding Base-N text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseNError {
    InvalidAlphabet { distinct_characters: usize },
    InvalidCharacter { character: char, position: usize },
}

impl fmt::Display for BaseNError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAlphabet {
                distinct_characters,
            } => write!(
                formatter,
                "alphabet must contain at least 2 distinct characters (found {distinct_characters})"
            ),
            Self::InvalidCharacter {
                character,
                position,
            } => write!(
                formatter,
                "input contains character {character:?} at position {position}, which is not in the alphabet"
            ),
        }
    }
}

impl Error for BaseNError {}

fn effective_alphabet(alphabet: &str) -> Result<Vec<char>, BaseNError> {
    let mut seen = HashSet::new();
    let symbols: Vec<char> = alphabet
        .chars()
        .filter(|character| seen.insert(*character))
        .collect();

    if symbols.len() < 2 {
        return Err(BaseNError::InvalidAlphabet {
            distinct_characters: symbols.len(),
        });
    }

    Ok(symbols)
}

/// Encodes big-endian bytes using positional conversion and the effective alphabet.
pub fn encode(alphabet: &str, input: &[u8]) -> Result<String, BaseNError> {
    let symbols = effective_alphabet(alphabet)?;
    let leading_zeroes = input.iter().take_while(|byte| **byte == 0).count();
    let mut number = BigUint::from_bytes_be(input);
    let base = BigUint::from(symbols.len());
    let mut digits = Vec::new();

    while !number.is_zero() {
        let remainder = (&number % &base)
            .to_usize()
            .expect("a remainder is always smaller than the alphabet length");
        digits.push(symbols[remainder]);
        number /= &base;
    }

    let mut output = String::new();
    output.extend(std::iter::repeat_n(symbols[0], leading_zeroes));
    output.extend(digits.into_iter().rev());
    Ok(output)
}

/// Decodes positional Base-N text into big-endian bytes.
pub fn decode(alphabet: &str, input: &str) -> Result<Vec<u8>, BaseNError> {
    let symbols = effective_alphabet(alphabet)?;
    let indexes: std::collections::HashMap<char, usize> = symbols
        .iter()
        .copied()
        .enumerate()
        .map(|(index, character)| (character, index))
        .collect();
    let zero = symbols[0];
    let leading_zeroes = input
        .chars()
        .take_while(|character| *character == zero)
        .count();
    let base = BigUint::from(symbols.len());
    let mut number = BigUint::zero();

    for (position, character) in input.chars().enumerate() {
        let Some(index) = indexes.get(&character) else {
            return Err(BaseNError::InvalidCharacter {
                character,
                position,
            });
        };
        number = number * &base + BigUint::from(*index);
    }

    let mut output = vec![0; leading_zeroes];
    if !number.is_zero() {
        output.extend(number.to_bytes_be());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{BaseNError, decode, encode};

    #[test]
    fn deduplicates_alphabet_in_first_occurrence_order() {
        assert_eq!(encode("ABABABA", &[5]).unwrap(), "BAB");
        assert_eq!(decode("ABABABA", "BAB").unwrap(), [5]);
    }

    #[test]
    fn preserves_leading_zero_bytes() {
        let input = [0, 0, 1, 2, 3];
        let encoded = encode(
            "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz",
            &input,
        )
        .unwrap();
        assert!(encoded.starts_with("11"));
        assert_eq!(
            decode(
                "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz",
                &encoded
            )
            .unwrap(),
            input
        );
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(encode("01", b"").unwrap(), "");
        assert_eq!(decode("01", "").unwrap(), b"");
    }

    #[test]
    fn rejects_invalid_alphabet_and_input() {
        assert_eq!(
            encode("AAAA", b"value"),
            Err(BaseNError::InvalidAlphabet {
                distinct_characters: 1
            })
        );
        assert!(matches!(
            decode("01", "012"),
            Err(BaseNError::InvalidCharacter { character: '2', .. })
        ));
    }
}
