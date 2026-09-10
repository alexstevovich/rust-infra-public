use crate::{FileNode, FileTree, NodeKind, ScanStatus};
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, c_void};
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

type Handle = *mut c_void;
const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const GENERIC_READ: u32 = 0x8000_0000;
const FILE_SHARE_ALL: u32 = 0x0000_0007;
const OPEN_EXISTING: u32 = 3;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const FSCTL_ENUM_USN_DATA: u32 = 0x0009_00b3;
const ERROR_HANDLE_EOF: u32 = 38;

#[repr(C)]
struct MftEnumData {
    start_file_reference_number: u64,
    low_usn: i64,
    high_usn: i64,
}

#[repr(C)]
#[derive(Default)]
struct ByHandleFileInformation {
    file_attributes: u32,
    creation_time: [u32; 2],
    last_access_time: [u32; 2],
    last_write_time: [u32; 2],
    volume_serial_number: u32,
    file_size_high: u32,
    file_size_low: u32,
    number_of_links: u32,
    file_index_high: u32,
    file_index_low: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *mut c_void,
        creation: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn DeviceIoControl(
        handle: Handle,
        code: u32,
        input: *mut c_void,
        input_len: u32,
        output: *mut c_void,
        output_len: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetLastError() -> u32;
    fn GetVolumePathNameW(file_name: *const u16, volume_path: *mut u16, length: u32) -> i32;
    fn GetVolumeNameForVolumeMountPointW(
        mount_point: *const u16,
        volume_name: *mut u16,
        length: u32,
    ) -> i32;
    fn GetVolumeInformationW(
        root: *const u16,
        volume_name: *mut u16,
        volume_name_len: u32,
        serial: *mut u32,
        max_component: *mut u32,
        flags: *mut u32,
        fs_name: *mut u16,
        fs_name_len: u32,
    ) -> i32;
    fn GetFileInformationByHandle(handle: Handle, info: *mut ByHandleFileInformation) -> i32;
}

#[derive(Debug)]
struct Record {
    parent: u64,
    name: String,
    attributes: u32,
}

struct OwnedHandle(Handle);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub fn scan_ntfs(root: &Path) -> io::Result<FileTree> {
    let root = root.canonicalize()?;
    let mount = volume_mount(&root)?;
    ensure_ntfs(&mount)?;
    let volume_name = volume_name(&mount)?;
    let volume = open(&volume_name, GENERIC_READ, 0)?;
    let root_handle = open(
        &root,
        0,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
    )?;
    let root_id = file_id(root_handle.0)?;
    let records = enumerate(volume.0)?;

    let mut children: HashMap<u64, Vec<u64>> = HashMap::new();
    for (&id, record) in &records {
        children.entry(record.parent).or_default().push(id);
    }
    for ids in children.values_mut() {
        ids.sort_by(|a, b| {
            records[a]
                .name
                .to_lowercase()
                .cmp(&records[b].name.to_lowercase())
        });
    }

    let metadata = std::fs::symlink_metadata(&root)?;
    let name = root
        .file_name()
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let mut seen = HashSet::new();
    let root_node = build_node(
        root_id,
        name,
        root,
        metadata.file_type().is_symlink(),
        metadata.is_dir(),
        &records,
        &children,
        &mut seen,
    );
    Ok(FileTree { root: root_node })
}

#[allow(clippy::too_many_arguments)]
fn build_node(
    id: u64,
    name: String,
    path: PathBuf,
    root_link: bool,
    root_dir: bool,
    records: &HashMap<u64, Record>,
    children: &HashMap<u64, Vec<u64>>,
    seen: &mut HashSet<u64>,
) -> FileNode {
    let record = records.get(&id);
    let is_dir = record.map_or(root_dir, |r| r.attributes & FILE_ATTRIBUTE_DIRECTORY != 0);
    let is_symlink = record.map_or(root_link, |r| {
        r.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    });
    let mut node = FileNode {
        name,
        path: path.clone(),
        kind: if is_dir {
            NodeKind::Directory
        } else {
            NodeKind::File
        },
        size: None,
        is_symlink,
        status: ScanStatus::Complete,
        children: Vec::new(),
    };
    if is_dir
        && !is_symlink
        && seen.insert(id)
        && let Some(ids) = children.get(&id)
    {
        for &child_id in ids {
            if let Some(child) = records.get(&child_id) {
                node.children.push(build_node(
                    child_id,
                    child.name.clone(),
                    path.join(&child.name),
                    false,
                    false,
                    records,
                    children,
                    seen,
                ));
            }
        }
    }
    node
}

fn enumerate(volume: Handle) -> io::Result<HashMap<u64, Record>> {
    let mut query = MftEnumData {
        start_file_reference_number: 0,
        low_usn: 0,
        high_usn: i64::MAX,
    };
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut records = HashMap::new();
    loop {
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                volume,
                FSCTL_ENUM_USN_DATA,
                &mut query as *mut _ as *mut c_void,
                std::mem::size_of::<MftEnumData>() as u32,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_HANDLE_EOF {
                break;
            }
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        if returned < 8 {
            break;
        }
        query.start_file_reference_number = u64::from_le_bytes(buffer[..8].try_into().unwrap());
        let mut offset = 8usize;
        while offset + 60 <= returned as usize {
            let length = read_u32(&buffer, offset) as usize;
            if length < 60 || offset + length > returned as usize {
                break;
            }
            let major = read_u16(&buffer, offset + 4);
            if major == 2 {
                let id = read_u64(&buffer, offset + 8);
                let parent = read_u64(&buffer, offset + 16);
                let attributes = read_u32(&buffer, offset + 52);
                let name_len = read_u16(&buffer, offset + 56) as usize;
                let name_offset = read_u16(&buffer, offset + 58) as usize;
                if name_offset + name_len <= length && name_len.is_multiple_of(2) {
                    let bytes = &buffer[offset + name_offset..offset + name_offset + name_len];
                    let units = bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|v| u16::from_le_bytes(*v))
                        .collect::<Vec<_>>();
                    records.insert(
                        id,
                        Record {
                            parent,
                            name: String::from_utf16_lossy(&units),
                            attributes,
                        },
                    );
                }
            }
            offset += length;
        }
    }
    Ok(records)
}

fn volume_mount(path: &Path) -> io::Result<PathBuf> {
    let input = wide(path.as_os_str());
    let mut output = vec![0u16; 1024];
    check(unsafe { GetVolumePathNameW(input.as_ptr(), output.as_mut_ptr(), output.len() as u32) })?;
    Ok(PathBuf::from(String::from_utf16_lossy(nul_slice(&output))))
}

fn volume_name(mount: &Path) -> io::Result<PathBuf> {
    let input = wide(mount.as_os_str());
    let mut output = vec![0u16; 1024];
    check(unsafe {
        GetVolumeNameForVolumeMountPointW(input.as_ptr(), output.as_mut_ptr(), output.len() as u32)
    })?;
    let value = String::from_utf16_lossy(nul_slice(&output));
    Ok(PathBuf::from(value.trim_end_matches(['\\', '/'])))
}

fn ensure_ntfs(mount: &Path) -> io::Result<()> {
    let input = wide(mount.as_os_str());
    let mut fs_name = [0u16; 32];
    check(unsafe {
        GetVolumeInformationW(
            input.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    })?;
    if String::from_utf16_lossy(nul_slice(&fs_name)).eq_ignore_ascii_case("NTFS") {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "MFT scanning requires an NTFS volume",
        ))
    }
}

fn open(path: &Path, access: u32, flags: u32) -> io::Result<OwnedHandle> {
    let value = wide(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            value.as_ptr(),
            access,
            FILE_SHARE_ALL,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            flags,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(OwnedHandle(handle))
    }
}

fn file_id(handle: Handle) -> io::Result<u64> {
    let mut info = ByHandleFileInformation::default();
    check(unsafe { GetFileInformationByHandle(handle, &mut info) })?;
    Ok((u64::from(info.file_index_high) << 32) | u64::from(info.file_index_low))
}

fn check(result: i32) -> io::Result<()> {
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn nul_slice(value: &[u16]) -> &[u16] {
    &value[..value.iter().position(|&v| v == 0).unwrap_or(value.len())]
}
fn read_u16(value: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(value[offset..offset + 2].try_into().unwrap())
}
fn read_u32(value: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(value[offset..offset + 4].try_into().unwrap())
}
fn read_u64(value: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(value[offset..offset + 8].try_into().unwrap())
}
