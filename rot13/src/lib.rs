//! ROT13 transformation for ASCII letters.

/// Applies ROT13 to ASCII letters while leaving every other character unchanged.
///
/// This function serves as both encode and decode because ROT13 is self-inverse:
/// applying it twice returns the original input. Separate operations are not needed.
pub fn apply(input: &str) -> String {
    input
        .chars()
        .map(|character| match character {
            'A'..='M' | 'a'..='m' => char::from_u32(character as u32 + 13).unwrap(),
            'N'..='Z' | 'n'..='z' => char::from_u32(character as u32 - 13).unwrap(),
            _ => character,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::apply;

    #[test]
    fn transforms_both_ascii_cases() {
        assert_eq!(apply("Hello, World!"), "Uryyb, Jbeyq!");
    }

    #[test]
    fn applying_twice_restores_input() {
        let input = "Tifa 123 — ティファ";
        assert_eq!(apply(&apply(input)), input);
    }

    #[test]
    fn preserves_non_ascii_letters() {
        assert_eq!(apply("café Ελληνικά"), "pnsé Ελληνικά");
    }
}
