use uefi::boot;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};
use uefi::CStr16;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_font::log_line_str;

const FILES_ARCHIVE: &str = "files.tar";

pub fn read_file(path: &str) -> alloc::vec::Vec<u8> {
    read_file_candidates(&[path])
}

/// Read a VFS path. Mutable user files live in the FAT-readable `files.tar`
/// archive; the old loose-file layout remains a read fallback for upgrades.
pub fn read_file_candidates(paths: &[&str]) -> alloc::vec::Vec<u8> {
    for path in paths {
        if let Some(data) = read_from_files_archive(path) {
            return data;
        }
    }
    read_direct_file_candidates(paths)
}

fn read_direct_file_candidates(paths: &[&str]) -> alloc::vec::Vec<u8> {
    // Strategy 1: try image handle's filesystem (works on QEMU)
    for path in paths {
        if let Some(data) = try_read_from_image_fs(path) {
            return data;
        }
    }

    // Strategy 2: enumerate all SimpleFileSystem handles (more robust for real hardware)
    try_read_from_any_fs(paths)
}

/// Translate the compatibility `/apps` namespace and the public `/files`
/// namespace into members of the on-disk archive.
fn archive_member(path: &str) -> Option<String> {
    let path = path.trim_start_matches('/');
    let member = if let Some(rest) = path.strip_prefix("apps/") {
        format!("app/{rest}")
    } else if let Some(rest) = path.strip_prefix("files/") {
        rest.to_string()
    } else if path.starts_with("data/") || path.starts_with("app/") {
        path.to_string()
    } else {
        return None;
    };
    is_safe_archive_path(&member).then_some(member)
}

fn is_safe_archive_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
}

fn read_from_files_archive(path: &str) -> Option<Vec<u8>> {
    let member = archive_member(path)?;
    let archive = read_direct_file_candidates(&[FILES_ARCHIVE]);
    read_archive_member(&archive, &member)
}

fn read_archive_member(archive: &[u8], wanted: &str) -> Option<Vec<u8>> {
    let mut offset = 0usize;
    while offset.checked_add(512)? <= archive.len() {
        let header = &archive[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            return None;
        }
        let name = tar_string(&header[0..100]);
        let prefix = tar_string(&header[345..500]);
        let full_name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let size = tar_octal(&header[124..136])?;
        let data_start = offset + 512;
        let data_end = data_start.checked_add(size)?;
        if data_end > archive.len() {
            return None;
        }
        let kind = header[156];
        if full_name.trim_start_matches("./") == wanted && kind != b'5' {
            return Some(archive[data_start..data_end].to_vec());
        }
        let padded = size.checked_add(511)? / 512 * 512;
        offset = data_start.checked_add(padded)?;
    }
    None
}

struct ArchiveEntry {
    name: String,
    data: Vec<u8>,
}

fn parse_archive(archive: &[u8]) -> Option<Vec<ArchiveEntry>> {
    let mut entries = Vec::new();
    let mut offset = 0usize;
    while offset.checked_add(512)? <= archive.len() {
        let header = &archive[offset..offset + 512];
        if header.iter().all(|byte| *byte == 0) {
            return Some(entries);
        }
        let name = tar_string(&header[0..100]);
        let prefix = tar_string(&header[345..500]);
        let full_name = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let size = tar_octal(&header[124..136])?;
        let data_start = offset + 512;
        let data_end = data_start.checked_add(size)?;
        let safe_name = full_name.trim_start_matches("./").trim_end_matches('/');
        if data_end > archive.len() || !is_safe_archive_path(safe_name) {
            return None;
        }
        if header[156] != b'5' {
            entries.push(ArchiveEntry {
                name: safe_name.into(),
                data: archive[data_start..data_end].to_vec(),
            });
        }
        let padded = size.checked_add(511)? / 512 * 512;
        offset = data_start.checked_add(padded)?;
    }
    None
}

fn build_archive(entries: &[ArchiveEntry]) -> Option<Vec<u8>> {
    let mut result = Vec::new();
    for entry in entries {
        if entry.name.len() > 100 || !is_safe_archive_path(&entry.name) {
            return None;
        }
        let mut header = [0u8; 512];
        header[..entry.name.len()].copy_from_slice(entry.name.as_bytes());
        write_octal(&mut header[100..108], 0o644);
        write_octal(&mut header[108..116], 0);
        write_octal(&mut header[116..124], 0);
        write_octal(&mut header[124..136], entry.data.len() as u64);
        write_octal(&mut header[136..148], 0);
        header[156] = b'0';
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        header[148..156].fill(b' ');
        let checksum: u32 = header.iter().map(|byte| *byte as u32).sum();
        write_checksum(&mut header[148..156], checksum);
        result.extend_from_slice(&header);
        result.extend_from_slice(&entry.data);
        let padding = (512 - (entry.data.len() % 512)) % 512;
        result.extend(core::iter::repeat(0u8).take(padding));
    }
    result.extend(core::iter::repeat(0u8).take(1024));
    Some(result)
}

fn write_octal(field: &mut [u8], value: u64) {
    field.fill(b'0');
    if field.is_empty() {
        return;
    }
    field[field.len() - 1] = 0;
    let mut value = value;
    let mut index = field.len().saturating_sub(2);
    while value != 0 && index < field.len() {
        field[index] = b'0' + (value as u8 & 7);
        value >>= 3;
        if index == 0 {
            break;
        }
        index -= 1;
    }
}

fn write_checksum(field: &mut [u8], value: u32) {
    field.fill(b' ');
    let mut value = value;
    for index in (0..6).rev() {
        field[index] = b'0' + (value as u8 & 7);
        value >>= 3;
    }
    field[6] = 0;
    field[7] = b' ';
}

fn tar_string(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().into()
}

fn tar_octal(bytes: &[u8]) -> Option<usize> {
    let mut value = 0usize;
    let mut found = false;
    for byte in bytes {
        match byte {
            b'0'..=b'7' => {
                value = value.checked_mul(8)?.checked_add((byte - b'0') as usize)?;
                found = true;
            }
            0 | b' ' => {}
            _ => return None,
        }
    }
    found.then_some(value)
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
    if let Some(member) = archive_member(path) {
        return write_archive_member(&member, data);
    }
    write_direct_file(path, data)
}

fn write_archive_member(member: &str, data: &[u8]) -> bool {
    let archive = read_direct_file_candidates(&[FILES_ARCHIVE]);
    let mut entries = if archive.is_empty() {
        Vec::new()
    } else {
        match parse_archive(&archive) {
            Some(entries) => entries,
            None => {
                log_line_str("VFS: files.tar is invalid; refusing to overwrite it");
                return false;
            }
        }
    };
    if let Some(entry) = entries.iter_mut().find(|entry| entry.name == member) {
        entry.data = data.to_vec();
    } else {
        entries.push(ArchiveEntry {
            name: member.into(),
            data: data.to_vec(),
        });
    }
    let Some(updated) = build_archive(&entries) else {
        log_line_str("VFS: cannot encode files.tar member");
        return false;
    };
    write_direct_file(FILES_ARCHIVE, &updated)
}

fn write_direct_file(path: &str, data: &[u8]) -> bool {
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
    if let Some(member) = archive_member(path) {
        let archive = read_direct_file_candidates(&[FILES_ARCHIVE]);
        let Some(mut entries) = parse_archive(&archive) else {
            return;
        };
        let original_len = entries.len();
        entries.retain(|entry| entry.name != member);
        if entries.len() != original_len {
            if let Some(updated) = build_archive(&entries) {
                let _ = write_direct_file(FILES_ARCHIVE, &updated);
            }
        }
        return;
    }
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
