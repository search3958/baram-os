use uefi::boot;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::CStr16;

use alloc::format;
use baram_font::log_line_str;

pub fn read_file(path: &str) -> alloc::vec::Vec<u8> {
    read_file_candidates(&[path])
}

/// Read the first existing path from the same filesystem search.  Application
/// archives use this for the `.w4a`/`.s4a` compatibility alias so a missing
/// preferred suffix does not produce a misleading VFS error before the valid
/// archive is tried.
pub fn read_file_candidates(paths: &[&str]) -> alloc::vec::Vec<u8> {
    // Strategy 1: try image handle's filesystem (works on QEMU)
    for path in paths {
        if let Some(data) = try_read_from_image_fs(path) {
            return data;
        }
    }

    // Strategy 2: enumerate all SimpleFileSystem handles (more robust for real hardware)
    try_read_from_any_fs(paths)
}

fn try_read_from_image_fs(path: &str) -> Option<alloc::vec::Vec<u8>> {
    let ih = uefi::boot::image_handle();
    let mut fs = uefi::boot::get_image_file_system(ih).ok()?;
    read_from_fs(&mut fs, path)
}

fn try_read_from_any_fs(paths: &[&str]) -> alloc::vec::Vec<u8> {
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

        for path in paths {
            if let Some(data) = read_from_fs(&mut fs, path) {
                log_line_str(&format!("VFS: found '{}' on fs handle #{}", path, idx));
                return data;
            }
        }
    }

    if let Some(path) = paths.first() {
        log_line_str(&format!("VFS: '{}' not found on any filesystem", path));
    }
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
                    if c == '\0' {
                        break;
                    }
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
    fs: &mut boot::ScopedProtocol<uefi::proto::media::fs::SimpleFileSystem>,
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

    let handle = root
        .open(cpath, FileMode::Read, FileAttribute::empty())
        .ok()?;
    let mut file = handle.into_regular_file()?;
    let mut info_buf = [0u8; 512];
    let file_size = match file.get_info::<uefi::proto::media::file::FileInfo>(&mut info_buf) {
        Ok(info) => info.file_size() as usize,
        Err(_) => 4096,
    };
    let mut contents = alloc::vec![0u8; file_size];
    match file.read(&mut contents) {
        Ok(n) => {
            contents.truncate(n);
        }
        Err(_) => {}
    }
    Some(contents)
}

pub fn read_file_str(path: &str) -> alloc::string::String {
    let bytes = read_file(path);
    alloc::string::String::from_utf8(bytes).unwrap_or_default()
}

pub fn write_file(path: &str, data: &[u8]) -> bool {
    let ih = uefi::boot::image_handle();
    if let Ok(fs) = uefi::boot::get_image_file_system(ih) {
        if write_to_fs(fs, path, data, true) {
            return true;
        }
    }

    // Some real firmware does not associate the loaded image with its FAT
    // volume reliably after the first write. Fall back only to a volume that
    // already contains the target, so an unrelated disk is never modified.
    if let Ok(handles) = boot::find_handles::<uefi::proto::media::fs::SimpleFileSystem>() {
        for handle in handles {
            let params = boot::OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            };
            if let Ok(fs) = unsafe {
                boot::open_protocol::<uefi::proto::media::fs::SimpleFileSystem>(
                    params,
                    boot::OpenProtocolAttributes::GetProtocol,
                )
            } {
                if write_to_fs(fs, path, data, false) {
                    return true;
                }
            }
        }
    }
    log_line_str(&format!(
        "VFS: write_file '{}' failed on every filesystem",
        path
    ));
    false
}

fn write_to_fs(
    mut fs: boot::ScopedProtocol<uefi::proto::media::fs::SimpleFileSystem>,
    path: &str,
    data: &[u8],
    create_if_missing: bool,
) -> bool {
    let Ok(mut root) = fs.open_volume() else {
        return false;
    };
    let mut path_buf = [0u16; 256];
    let mut path_len = 0;
    for ch in path.bytes() {
        if path_len + 1 >= path_buf.len() {
            return false;
        }
        path_buf[path_len] = if ch == b'/' { b'\\' } else { ch } as u16;
        path_len += 1;
    }
    path_buf[path_len] = 0;
    let Ok(cpath) = CStr16::from_u16_with_nul(&path_buf[..=path_len]) else {
        return false;
    };

    let handle = match root.open(cpath, FileMode::ReadWrite, FileAttribute::empty()) {
        Ok(handle) => handle,
        Err(_) if create_if_missing => {
            match root.open(cpath, FileMode::CreateReadWrite, FileAttribute::empty()) {
                Ok(handle) => handle,
                Err(error) => {
                    log_line_str(&format!("VFS: create '{}' failed: {:?}", path, error));
                    return false;
                }
            }
        }
        Err(_) => return false,
    };
    let Some(mut file) = handle.into_regular_file() else {
        return false;
    };

    // Never delete the live config before its replacement is durable. Several
    // real UEFI FAT drivers become unusable after rapid delete/create cycles.
    // Write from offset zero, then truncate the old tail through SetInfo.
    if let Err(error) = file.set_position(0) {
        log_line_str(&format!("VFS: seek '{}' failed: {:?}", path, error));
        return false;
    }
    if let Err(error) = file.write(data) {
        log_line_str(&format!(
            "VFS: write_file '{}' incomplete: {}/{}",
            path,
            error.data(),
            data.len()
        ));
        return false;
    }

    let Ok(old_info) = file.get_boxed_info::<FileInfo>() else {
        log_line_str(&format!("VFS: metadata '{}' failed", path));
        return false;
    };
    let mut info_words = [0u64; 128];
    let info_bytes = unsafe {
        core::slice::from_raw_parts_mut(
            info_words.as_mut_ptr().cast::<u8>(),
            core::mem::size_of_val(&info_words),
        )
    };
    let Ok(new_info) = FileInfo::new(
        info_bytes,
        data.len() as u64,
        old_info.physical_size(),
        *old_info.create_time(),
        *old_info.last_access_time(),
        *old_info.modification_time(),
        old_info.attribute(),
        old_info.file_name(),
    ) else {
        log_line_str(&format!("VFS: metadata buffer '{}' failed", path));
        return false;
    };
    if let Err(error) = file.set_info(new_info) {
        log_line_str(&format!("VFS: truncate '{}' failed: {:?}", path, error));
        return false;
    }
    if let Err(error) = file.flush() {
        log_line_str(&format!("VFS: flush '{}' failed: {:?}", path, error));
        return false;
    }
    true
}

pub fn remove_file(path: &str) {
    let ih = uefi::boot::image_handle();
    if let Ok(mut fs) = uefi::boot::get_image_file_system(ih) {
        if let Ok(mut root) = fs.open_volume() {
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
            if let Ok(cpath) = CStr16::from_u16_with_nul(&buf[..=i]) {
                if let Ok(handle) = root.open(cpath, FileMode::Read, FileAttribute::empty()) {
                    let _ = handle.delete();
                }
            }
        }
    }
}
