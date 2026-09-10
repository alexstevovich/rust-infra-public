use alexstevovich_uuidv5::{NAMESPACE_DNS, NAMESPACE_OID, NAMESPACE_URL, NAMESPACE_X500, Uuid};
use std::process::ExitCode;

fn usage() {
    eprintln!("Usage: uuidv5 <dns|url|oid|x500|uuid> <name>");
}

fn parse_namespace(value: &str) -> Result<Uuid, String> {
    match value.to_ascii_lowercase().as_str() {
        "dns" => Ok(NAMESPACE_DNS),
        "url" => Ok(NAMESPACE_URL),
        "oid" => Ok(NAMESPACE_OID),
        "x500" => Ok(NAMESPACE_X500),
        _ => Uuid::parse_str(value).map_err(|error| error.to_string()),
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(namespace), Some(name), None) = (args.next(), args.next(), args.next()) else {
        usage();
        return ExitCode::from(2);
    };

    let namespace = match parse_namespace(&namespace) {
        Ok(namespace) => namespace,
        Err(error) => {
            eprintln!("Invalid namespace: {error}");
            return ExitCode::FAILURE;
        }
    };

    println!("{}", alexstevovich_uuidv5::generate(namespace, &name));
    ExitCode::SUCCESS
}
