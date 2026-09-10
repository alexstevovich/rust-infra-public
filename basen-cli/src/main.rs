use std::{io::Write, process::ExitCode};

fn usage() {
    eprintln!("Usage: basen <encode|decode> <alphabet> <value>");
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(operation), Some(alphabet), Some(value), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        usage();
        return ExitCode::from(2);
    };

    match operation.as_str() {
        "encode" => match alexstevovich_basen::encode(&alphabet, value.as_bytes()) {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("Base-N error: {error}");
                return ExitCode::FAILURE;
            }
        },
        "decode" => match alexstevovich_basen::decode(&alphabet, &value) {
            Ok(output) => {
                if let Err(error) = std::io::stdout().write_all(&output) {
                    eprintln!("Error writing output: {error}");
                    return ExitCode::FAILURE;
                }
            }
            Err(error) => {
                eprintln!("Base-N error: {error}");
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
