use crate::FileTree;
use std::io::Write;
use std::path::Path;

pub const INTERNAL_ELEVATED_SCAN_ARG: &str = "__filetree-maker-elevated";

#[derive(Debug)]
pub enum ElevationError {
    Unsupported,
    Io(std::io::Error),
    Serialize(serde_json::Error),
    Scan(crate::ScanError),
    InvalidArguments,
    LaunchFailed(String),
    HelperFailed(u32),
}

impl std::fmt::Display for ElevationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => write!(f, "elevation is only supported on Windows"),
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Serialize(error) => write!(f, "serialization error: {error}"),
            Self::Scan(error) => write!(f, "scan failed: {error}"),
            Self::InvalidArguments => write!(f, "invalid elevated scan arguments"),
            Self::LaunchFailed(message) => {
                write!(f, "could not launch elevated scanner: {message}")
            }
            Self::HelperFailed(code) => write!(f, "elevated scanner exited with code {code}"),
        }
    }
}

impl std::error::Error for ElevationError {}

impl From<std::io::Error> for ElevationError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for ElevationError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialize(value)
    }
}

impl From<crate::ScanError> for ElevationError {
    fn from(value: crate::ScanError) -> Self {
        Self::Scan(value)
    }
}

pub fn is_elevated_scan_command(args: &[String]) -> bool {
    args.get(1)
        .is_some_and(|arg| arg == INTERNAL_ELEVATED_SCAN_ARG)
}

pub fn run_elevated_scan(args: &[String]) -> Result<(), ElevationError> {
    let root = args.get(2).ok_or(ElevationError::InvalidArguments)?;
    let output = args.get(3).ok_or(ElevationError::InvalidArguments)?;
    let output_path = Path::new(output);
    let valid_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("filetree-") && name.ends_with("-scan.json"));
    let valid_parent = output_path
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .zip(std::env::temp_dir().canonicalize().ok())
        .is_some_and(|(parent, temp)| parent == temp);
    if !valid_name || !valid_parent {
        return Err(ElevationError::InvalidArguments);
    }
    #[cfg(windows)]
    let tree = crate::ntfs::scan_ntfs(Path::new(root)).map_err(ElevationError::Io)?;
    #[cfg(not(windows))]
    let tree = {
        let _ = (root, output);
        return Err(ElevationError::Unsupported);
    };
    let json = serde_json::to_vec_pretty(&tree)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output_path)?
        .write_all(&json)?;
    Ok(())
}

#[cfg(windows)]
fn request_elevated_scan_to(root: &Path, output: &Path) -> Result<(), ElevationError> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
    use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
    use windows::core::PCWSTR;

    fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
        value
            .as_ref()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let exe = std::env::current_exe()?;
    let quote = |value: &Path| format!("\"{}\"", value.display().to_string().replace('"', "\\\""));
    let params = format!(
        "{} {} {}",
        INTERNAL_ELEVATED_SCAN_ARG,
        quote(root),
        quote(output)
    );

    let exe_w = wide(exe.as_os_str());
    let verb_w = wide("runas");
    let params_w = wide(params);

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR(verb_w.as_ptr()),
        lpFile: PCWSTR(exe_w.as_ptr()),
        lpParameters: PCWSTR(params_w.as_ptr()),
        lpDirectory: PCWSTR::null(),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }
        .map_err(|error| ElevationError::LaunchFailed(error.to_string()))?;
    if info.hProcess.is_invalid() {
        return Err(ElevationError::LaunchFailed(
            "no process handle returned".into(),
        ));
    }
    let wait = unsafe { WaitForSingleObject(info.hProcess, INFINITE) };
    if wait != WAIT_OBJECT_0 {
        unsafe { CloseHandle(info.hProcess) }.ok();
        return Err(ElevationError::LaunchFailed(format!(
            "wait failed: {wait:?}"
        )));
    }
    let mut exit_code = 0;
    let exit_result = unsafe { GetExitCodeProcess(info.hProcess, &mut exit_code) };
    unsafe { CloseHandle(info.hProcess) }.ok();
    exit_result.map_err(|error| ElevationError::LaunchFailed(error.to_string()))?;
    if exit_code != 0 {
        return Err(ElevationError::HelperFailed(exit_code));
    }
    Ok(())
}

#[cfg(not(windows))]
fn request_elevated_scan_to(_root: &Path, _output: &Path) -> Result<(), ElevationError> {
    Err(ElevationError::Unsupported)
}

pub fn request_elevated_scan(root: &Path) -> Result<FileTree, ElevationError> {
    let root = root.canonicalize()?;
    let output = std::env::temp_dir().join(format!(
        "filetree-{}-{}-scan.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let result = (|| {
        request_elevated_scan_to(&root, &output)?;
        let data = std::fs::read(&output)?;
        Ok(serde_json::from_slice(&data)?)
    })();
    let _ = std::fs::remove_file(output);
    result
}
