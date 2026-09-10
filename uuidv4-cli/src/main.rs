use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args_os().nth(1).is_some() {
        eprintln!("Usage: uuidv4 (takes no arguments)");
        return ExitCode::from(2);
    }

    println!("{}", alexstevovich_uuidv4::generate());
    ExitCode::SUCCESS
}
