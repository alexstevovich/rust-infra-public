use std::{error::Error, fs, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(source), Some(target), None) = (args.next(), args.next(), args.next()) else {
        eprintln!("Usage: a1111-meta-copy <source> <target>");
        return ExitCode::from(2);
    };

    match copy(Path::new(&source), Path::new(&target)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("a1111-meta-copy: {error}");
            ExitCode::FAILURE
        }
    }
}

fn copy(source: &Path, target: &Path) -> Result<(), Box<dyn Error>> {
    let source = fs::read(source)?;
    let Some(metadata) = a1111_meta::extract(&source)? else {
        return Ok(());
    };
    let target_bytes = fs::read(target)?;
    let updated = a1111_meta::embed(&target_bytes, &metadata)?;
    image_container_meta::write_atomic(target, &updated)?;
    Ok(())
}
