use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(value) = std::env::args().nth(1) else {
        eprintln!("Usage: sha512 <value>");
        return ExitCode::from(2);
    };

    let digest = alexstevovich_sha512::hash(value.as_bytes());
    println!("{}", alexstevovich_sha512::to_hex(&digest));
    ExitCode::SUCCESS
}
