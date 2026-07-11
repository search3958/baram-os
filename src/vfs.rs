use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::boot;
use uefi::CStr16;

use crate::mouse::log_line_str;
use alloc::format;

pub fn read_file(path: &str) -> alloc::vec::Vec<u8> {
    // Strategy 1: try image handle's filesystem (works on QEMU)
    if let Some(data) = try_read_from_image_fs(path) {
        return data;
    }

    // Strategy 2: enumerate all SimpleFileSystem handles (more robust for real hardware)
    try_read_from_any_fs(path)
}

fn try_read_from_image_fs(path: &str) -> Option<alloc::vec::Vec<u8>> {
    let ih = uefi::boot::image_handle();
    let fs = uefi::boot::get_image_file_system(ih).ok()?;
    read_from_fs(fs, path)
}

fn try_read_from_any_fs(path: &str) -> alloc::vec::Vec<u8> {
    let handles = match boot::find_handles::<uefi::proto::media::fs::SimpleFileSystem>() {
        Ok(h) => h,
        Err(_) => {
            log_line_str("VFS: no SimpleFileSystem handles found");
            return alloc::vec::Vec::new();
        }
    };

    log_line_str(&format!("VFS: found {} filesystem handles", handles.len()));

    for (idx, handle) in handles.iter().enumerate() {
        let params = boot::OpenProtocolParams {
            handle: *handle,
            agent: boot::image_handle(),
            controller: None,
        };
        let mut fs = match unsafe {
            boot::open_protocol::<uefi::proto::media::fs::SimpleFileSystem>(
                params,
                boot::OpenProtocolAttributes::GetProtocol,
            )
        } {
            Ok(f) => f,
            Err(_) => continue,
        };

        // List root directory contents for debugging
        if let Ok(mut root) = fs.open_volume() {
            list_dir(&mut root, "", idx);
        }

        if let Some(data) = read_from_fs(fs, path) {
            log_line_str(&format!("VFS: found '{}' on fs handle #{}", path, idx));
            return data;
        }
    }

    log_line_str(&format!("VFS: '{}' not found on any filesystem", path));
    alloc::vec::Vec::new()
}

fn list_dir(root: &mut uefi::proto::media::file::Directory, prefix: &str, fs_idx: usize) {
    let mut buf = [0u8; 256];
    loop {
        match root.read_entry(&mut buf) {
            Ok(Some(entry)) => {
                let name_utf16 = entry.file_name().as_slice();
                let mut name = alloc::string::String::new();
                for &ch in name_utf16 {
                    let c: char = ch.into();
                    if c == '\0' { break; }
                    name.push(c);
                }
                let full = if prefix.is_empty() {
                    name.clone()
                } else {
                    alloc::format!("{}/{}", prefix, name)
                };
                log_line_str(&format!("  fs#{}: {}", fs_idx, full));
            }
            _ => break,
        }
    }
}

fn read_from_fs(
    mut fs: boot::ScopedProtocol<uefi::proto::media::fs::SimpleFileSystem>,
    path: &str,
) -> Option<alloc::vec::Vec<u8>> {
    let mut root = fs.open_volume().ok()?;

    // UEFI paths use backslash
    let mut buf = [0u16; 256];
    let mut i = 0;
    for ch in path.bytes() {
        let c = if ch == b'/' { b'\\' } else { ch } as u16;
        if i + 1 < buf.len() {
            buf[i] = c;
            i += 1;
        }
    }
    buf[i] = 0;
    let cpath = CStr16::from_u16_with_nul(&buf[..=i]).ok()?;

    let handle = root.open(cpath, FileMode::Read, FileAttribute::empty()).ok()?;
    let mut file = handle.into_regular_file()?;
    let mut info_buf = [0u8; 512];
    let file_size = match file.get_info::<uefi::proto::media::file::FileInfo>(&mut info_buf) {
        Ok(info) => info.file_size() as usize,
        Err(_) => 4096,
    };
    let mut contents = alloc::vec![0u8; file_size];
    match file.read(&mut contents) {
        Ok(n) => { contents.truncate(n); }
        Err(_) => {}
    }
    Some(contents)
}

pub fn read_file_str(path: &str) -> alloc::string::String {
    let bytes = read_file(path);
    alloc::string::String::from_utf8(bytes).unwrap_or_default()
}
