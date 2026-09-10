//! Pure UUID version 4 generation.

pub use uuid::Uuid;

/// Generates a random UUID version 4.
pub fn generate() -> Uuid {
    Uuid::new_v4()
}

#[cfg(test)]
mod tests {
    use super::generate;

    #[test]
    fn generates_version_four_uuid() {
        let uuid = generate();
        assert_eq!(uuid.get_version_num(), 4);
        assert_eq!(uuid.to_string().len(), 36);
    }
}
