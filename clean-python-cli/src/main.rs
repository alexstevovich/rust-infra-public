use std::process::ExitCode;

fn usage() {
    eprintln!("Usage: clean-python <root> [--recursive] [--dry-run] [--venv]");
}

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(root) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let mut recursive = false;
    let mut dry_run = false;
    let mut include_venv = false;
    for argument in args {
        match argument.to_str() {
            Some("--recursive") => recursive = true,
            Some("--dry-run") => dry_run = true,
            Some("--venv") => include_venv = true,
            _ => {
                usage();
                return ExitCode::from(2);
            }
        }
    }
    let report = alexstevovich_clean_python::clean(root, recursive, dry_run, include_venv);
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
