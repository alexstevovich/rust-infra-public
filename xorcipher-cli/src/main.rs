use std::{fmt::Write, process::ExitCode};

fn usage() {
    eprintln!("Usage: xorcipher <key> <value>");
    eprintln!("Both arguments are interpreted as UTF-8 text; output is lowercase hex.");
    eprintln!(
        "To reverse the operation, convert the hex output to raw bytes before applying the same key again."
    );
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(key), Some(value), None) = (args.next(), args.next(), args.next()) else {
        usage();
        return ExitCode::from(2);
    };

    let output = alexstevovich_xorcipher::apply(key.as_bytes(), value.as_bytes());
    let mut hex = String::with_capacity(output.len() * 2);
    for byte in output {
        write!(&mut hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    println!("{hex}");
    ExitCode::SUCCESS
}
