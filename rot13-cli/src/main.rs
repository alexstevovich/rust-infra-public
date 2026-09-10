use std::process::ExitCode;

fn usage() {
    eprintln!("Usage: rot13 <value>");
    eprintln!("Encoding and decoding are identical — run rot13 again on the output to reverse it.");
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let value = args.next();
    let extra = args.next();

    if value.as_deref() == Some("--help") && extra.is_none() {
        usage();
        return ExitCode::SUCCESS;
    }

    let (Some(value), None) = (value, extra) else {
        usage();
        return ExitCode::from(2);
    };

    println!("{}", alexstevovich_rot13::apply(&value));
    ExitCode::SUCCESS
}
