use alexstevovich_json_bundle as json_bundle;
use clap::{Parser, Subcommand};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "json-bundle",
    version,
    about = "Reversibly bundle JSON objects using their absolute source paths"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pack JSON object files into one array. The final path is the output.
    Pack {
        #[arg(required = true, num_args = 2.., value_name = "PATH")]
        paths: Vec<String>,
        #[arg(long, default_value = json_bundle::DEFAULT_PATH_KEY)]
        path_key: String,
    },
    /// Write bundled objects back to their recorded absolute paths.
    Unpack {
        bundle: PathBuf,
        #[arg(long, default_value = json_bundle::DEFAULT_PATH_KEY)]
        path_key: String,
    },
}

fn main() -> ExitCode {
    let result = match Args::parse().command {
        Command::Pack {
            mut paths,
            path_key,
        } => {
            let output = PathBuf::from(paths.pop().expect("clap requires an output path"));
            json_bundle::pack(&paths, &output, &path_key)
                .map(|count| println!("packed {count} JSON object(s) into {}", output.display()))
        }
        Command::Unpack { bundle, path_key } => json_bundle::unpack(&bundle, &path_key)
            .map(|count| println!("unpacked {count} JSON object(s) from {}", bundle.display())),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("json-bundle: {error}");
            ExitCode::FAILURE
        }
    }
}
