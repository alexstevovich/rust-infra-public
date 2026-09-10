use alexstevovich_nanoid::DEFAULT_ALPHABET;
use std::process::ExitCode;

fn usage() {
    eprintln!("Usage: nanoid [length [alphabet]]");
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let length = args.next();
    let alphabet = args.next();
    if args.next().is_some() {
        usage();
        return ExitCode::from(2);
    }

    if length.is_none() {
        println!("{}", alexstevovich_nanoid::generate_default());
        return ExitCode::SUCCESS;
    }

    let length = match length.unwrap().parse::<usize>() {
        Ok(length) => length,
        Err(error) => {
            eprintln!("Invalid length: {error}");
            return ExitCode::FAILURE;
        }
    };
    let custom_alphabet;
    let alphabet = match alphabet {
        Some(value) => {
            custom_alphabet = value.chars().collect::<Vec<_>>();
            custom_alphabet.as_slice()
        }
        None => &DEFAULT_ALPHABET,
    };

    match alexstevovich_nanoid::generate(length, alphabet) {
        Ok(id) => {
            println!("{id}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Nano ID error: {error}");
            ExitCode::FAILURE
        }
    }
}
