//! Pure UUID version 5 generation and standard RFC namespace constants.

pub use uuid::Uuid;

pub const NAMESPACE_DNS: Uuid = Uuid::NAMESPACE_DNS;
pub const NAMESPACE_URL: Uuid = Uuid::NAMESPACE_URL;
pub const NAMESPACE_OID: Uuid = Uuid::NAMESPACE_OID;
pub const NAMESPACE_X500: Uuid = Uuid::NAMESPACE_X500;

/// Generates a deterministic UUID version 5 from a namespace and name.
pub fn generate(namespace: Uuid, name: &str) -> Uuid {
    Uuid::new_v5(&namespace, name.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{NAMESPACE_DNS, generate};

    #[test]
    fn matches_rfc_dns_vector() {
        assert_eq!(
            generate(NAMESPACE_DNS, "www.example.com").to_string(),
            "2ed6657d-e927-568b-95e1-2665a8aea6a2"
        );
    }
}
