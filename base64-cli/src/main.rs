use std::{io::Write, process::ExitCode};

fn usage() {
    eprintln!("Usage: base64 <encode|decode> <value>");
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(operation), Some(value), None) = (args.next(), args.next(), args.next()) else {
        usage();
        return ExitCode::from(2);
    };

    match operation.as_str() {
        "encode" => println!("{}", alexstevovich_base64::encode(value.as_bytes())),
        "decode" => match alexstevovich_base64::decode(&value) {
            Ok(bytes) => {
                if let Err(error) = std::io::stdout().write_all(&bytes) {
                    eprintln!("Error writing output: {error}");
                    return ExitCode::FAILURE;
                }
            }
            Err(error) => {
                eprintln!("Invalid Base64 input: {error}");
                return ExitCode::FAILURE;
            }
        },
        _ => {
            usage();
            return ExitCode::from(2);
        }
    }

    ExitCode::SUCCESS
}
