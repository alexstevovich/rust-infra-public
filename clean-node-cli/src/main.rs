use std::process::ExitCode;

fn usage() {
    eprintln!("Usage: clean-node <root> [--recursive] [--dry-run]");
}

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(root) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let mut recursive = false;
    let mut dry_run = false;
    for argument in args {
        match argument.to_str() {
            Some("--recursive") => recursive = true,
            Some("--dry-run") => dry_run = true,
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    let report = alexstevovich_clean_node::clean(root, recursive, dry_run);
    for path in report.paths {
        println!("{}", path.display());
    }
    for error in &report.errors {
        eprintln!("{}: {}", error.path.display(), error.error);
    }
    if report.errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
