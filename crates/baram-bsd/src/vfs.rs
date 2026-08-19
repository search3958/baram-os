use uefi::boot;
use uefi::proto::media::file::{Directory, File, FileAttribute, FileInfo, FileMode, RegularFile};
use uefi::CStr16;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use baram_font::log_line_str;

const FILES_ARCHIVE: &str = "files.tar";

pub fn read_file(path: &str) -> alloc::vec::Vec<u8> {
    read_file_candidates(&[path])
}

/// Read a VFS path from the normal FAT directory tree. The legacy `files.tar`
/// layout remains a read fallback for older disk images.
pub fn read_file_candidates(paths: &[&str]) -> alloc::vec::Vec<u8> {
    // The current image layout is a regular filesystem. Try it first so a
    // lookup does not scan an archive from its beginning.
    for path in paths {
        if let Some(mapped) = direct_fs_path(path) {
            if let Some(data) = try_read_from_image_fs(&mapped) {
                return data;
            }
        }
    }

    // Compatibility path for existing images which still contain files.tar.
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
        let Some(mapped) = direct_fs_path(path) else {
            continue;
        };
        if let Some(data) = try_read_from_image_fs(&mapped) {
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

/// Map the public VFS namespace to the regular files stored on the FAT
/// volume. `apps/foo` historically meant the archive member `app/foo`; in the
/// directory layout it is `files/app/foo`.
fn direct_fs_path(path: &str) -> Option<String> {
    let path = path.trim_start_matches('/').trim_end_matches('/');
    if path.is_empty() {
        return Some(String::new());
    }
    if !is_safe_archive_path(path) {
        return None;
    }
    if let Some(rest) = path.strip_prefix("apps/") {
        return Some(format!("files/app/{rest}"));
    }
    if path == "apps" {
        return Some("files/app".into());
    }
    if path.starts_with("files/") || path == "files" {
        return Some(path.into());
    }
    if path.starts_with("app/") || path.starts_with("data/") {
        return Some(format!("files/{path}"));
    }
    Some(path.into())
}

fn read_from_files_archive(path: &str) -> Option<Vec<u8>> {
    let member = archive_member(path)?;
    try_read_archive_member_from_image_fs(&member)
        .or_else(|| try_read_archive_member_from_any_fs(&member))
}

/// Read one TAR member without ever materializing `files.tar`. The archive is
/// scanned in-place with a 512-byte header and a small skip buffer; only the
/// requested member is returned to the caller.
fn read_archive_member_from_fs(
    fs: &mut boot::ScopedProtocol<uefi::proto::media::fs::SimpleFileSystem>,
    wanted: &str,
) -> Option<Vec<u8>> {
    let mut file = open_regular_file(fs, FILES_ARCHIVE)?;
    let mut header = [0u8; 512];
    loop {
        if !read_exact(&mut file, &mut header) {
            return None;
        }
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
        let padded = size.checked_add(511)? / 512 * 512;
        let kind = header[156];
        if full_name.trim_start_matches("./") == wanted && kind != b'5' {
            let mut data = alloc::vec![0u8; size];
            if !read_exact(&mut file, &mut data) {
                return None;
            }
            return Some(data);
        }
        if !skip_bytes(&mut file, padded) {
            return None;
        }
    }
}

fn try_read_archive_member_from_image_fs(wanted: &str) -> Option<Vec<u8>> {
    let image = uefi::boot::image_handle();
    let mut fs = uefi::boot::get_image_file_system(image).ok()?;
    read_archive_member_from_fs(&mut fs, wanted)
}

fn try_read_archive_member_from_any_fs(wanted: &str) -> Option<Vec<u8>> {
    let handles = boot::find_handles::<uefi::proto::media::fs::SimpleFileSystem>().ok()?;
    for handle in handles {
        let params = boot::OpenProtocolParams {
            handle,
            agent: boot::image_handle(),
            controller: None,
        };
        let Ok(mut fs) = (unsafe {
            boot::open_protocol::<uefi::proto::media::fs::SimpleFileSystem>(
                params,
                boot::OpenProtocolAttributes::GetProtocol,
            )
        }) else {
            continue;
        };
        if let Some(data) = read_archive_member_from_fs(&mut fs, wanted) {
            return Some(data);
        }
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
            let Some(mapped) = direct_fs_path(path) else {
                continue;
            };
            if let Some(data) = read_from_fs(&mut fs, &mapped) {
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
    let mut file = open_regular_file(fs, path)?;
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

fn open_regular_file(
    fs: &mut boot::ScopedProtocol<uefi::proto::media::fs::SimpleFileSystem>,
    path: &str,
) -> Option<RegularFile> {
    let mut root = fs.open_volume().ok()?;
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
    root.open(cpath, FileMode::Read, FileAttribute::empty())
        .ok()?
        .into_regular_file()
}

fn open_directory(
    fs: &mut boot::ScopedProtocol<uefi::proto::media::fs::SimpleFileSystem>,
    path: &str,
) -> Option<Directory> {
    let mut root = fs.open_volume().ok()?;
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
    root.open(cpath, FileMode::Read, FileAttribute::empty())
        .ok()?
        .into_directory()
}

fn read_exact(file: &mut RegularFile, buffer: &mut [u8]) -> bool {
    let mut offset = 0usize;
    while offset < buffer.len() {
        match file.read(&mut buffer[offset..]) {
            Ok(0) | Err(_) => return false,
            Ok(read) => offset = offset.saturating_add(read),
        }
    }
    true
}

fn skip_bytes(file: &mut RegularFile, count: usize) -> bool {
    let Ok(position) = file.get_position() else {
        return false;
    };
    let Some(position) = position.checked_add(count as u64) else {
        return false;
    };
    file.set_position(position).is_ok()
}

pub fn read_file_str(path: &str) -> alloc::string::String {
    let bytes = read_file(path);
    alloc::string::String::from_utf8(bytes).unwrap_or_default()
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
}

/// Convert a `files://` URI into the VFS namespace used by the archive.
pub fn parse_files_uri(uri: &str) -> Option<String> {
    let path = uri.trim().strip_prefix("files://")?.trim_start_matches('/');
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        return Some("files/".into());
    }
    if !is_safe_archive_path(path) {
        return None;
    }
    Some(format!("files/{path}"))
}

/// List the immediate children of an archive directory. Directory entries are
/// inferred from member prefixes because the compact TAR writer omits empty
/// directory records when rewriting the archive.
pub fn list_files(path: &str) -> Vec<FileEntry> {
    let vfs_path = parse_files_uri(path).unwrap_or_else(|| path.into());
    let direct_path = direct_fs_path(&vfs_path).unwrap_or_default();
    let mut result = Vec::new();
    let found = try_list_direct_from_image_fs(&direct_path, &mut result)
        || try_list_direct_from_any_fs(&direct_path, &mut result);
    if found {
        result.sort_by(|a, b| a.name.cmp(&b.name));
        return result;
    }

    let prefix = if vfs_path.trim_end_matches('/') == "files" {
        String::new()
    } else {
        let Some(directory) = archive_member(&vfs_path) else {
            return Vec::new();
        };
        format!("{}/", directory.trim_end_matches('/'))
    };
    let found = try_list_archive_from_image_fs(&prefix, &mut result)
        || try_list_archive_from_any_fs(&prefix, &mut result);
    if !found {
        return Vec::new();
    }
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

fn list_direct_from_fs(
    fs: &mut boot::ScopedProtocol<uefi::proto::media::fs::SimpleFileSystem>,
    path: &str,
    result: &mut Vec<FileEntry>,
) -> bool {
    let Some(mut directory) = open_directory(fs, path) else {
        return false;
    };
    let mut buf = [0u8; 512];
    loop {
        match directory.read_entry(&mut buf) {
            Ok(Some(entry)) => {
                let name_utf16 = entry.file_name().as_slice();
                let mut name = String::new();
                for &ch in name_utf16 {
                    let c: char = ch.into();
                    if c == '\0' {
                        break;
                    }
                    name.push(c);
                }
                if !name.is_empty() && name != "." && name != ".." {
                    result.push(FileEntry {
                        name,
                        is_dir: entry.is_directory(),
                    });
                }
            }
            Ok(None) => return true,
            Err(_) => return false,
        }
    }
}

fn try_list_direct_from_image_fs(path: &str, result: &mut Vec<FileEntry>) -> bool {
    let image = uefi::boot::image_handle();
    let Ok(mut fs) = uefi::boot::get_image_file_system(image) else {
        return false;
    };
    list_direct_from_fs(&mut fs, path, result)
}

fn try_list_direct_from_any_fs(path: &str, result: &mut Vec<FileEntry>) -> bool {
    let Ok(handles) = boot::find_handles::<uefi::proto::media::fs::SimpleFileSystem>() else {
        return false;
    };
    for handle in handles {
        let params = boot::OpenProtocolParams {
            handle,
            agent: boot::image_handle(),
            controller: None,
        };
        let Ok(mut fs) = (unsafe {
            boot::open_protocol::<uefi::proto::media::fs::SimpleFileSystem>(
                params,
                boot::OpenProtocolAttributes::GetProtocol,
            )
        }) else {
            continue;
        };
        if list_direct_from_fs(&mut fs, path, result) {
            return true;
        }
    }
    false
}

fn list_archive_from_fs(
    fs: &mut boot::ScopedProtocol<uefi::proto::media::fs::SimpleFileSystem>,
    prefix: &str,
    result: &mut Vec<FileEntry>,
) -> bool {
    let mut file = match open_regular_file(fs, FILES_ARCHIVE) {
        Some(file) => file,
        None => return false,
    };
    let mut header = [0u8; 512];
    loop {
        if !read_exact(&mut file, &mut header) {
            return false;
        }
        if header.iter().all(|byte| *byte == 0) {
            return true;
        }
        let name = tar_string(&header[0..100]);
        let prefix_name = tar_string(&header[345..500]);
        let full_name = if prefix_name.is_empty() {
            name
        } else {
            format!("{prefix_name}/{name}")
        };
        let size = match tar_octal(&header[124..136]) {
            Some(size) => size,
            None => return false,
        };
        let member = full_name.trim_start_matches("./").trim_end_matches('/');
        if let Some(rest) = member.strip_prefix(prefix) {
            let (entry_name, is_dir) = match rest.split_once('/') {
                Some((entry_name, _)) => (entry_name, true),
                None => (rest, header[156] == b'5'),
            };
            if !entry_name.is_empty()
                && !result.iter().any(|item| item.name == entry_name)
            {
                result.push(FileEntry {
                    name: entry_name.into(),
                    is_dir,
                });
            }
        }
        let padded = match size.checked_add(511) {
            Some(size) => size / 512 * 512,
            None => return false,
        };
        if !skip_bytes(&mut file, padded) {
            return false;
        }
    }
}

fn try_list_archive_from_image_fs(prefix: &str, result: &mut Vec<FileEntry>) -> bool {
    let image = uefi::boot::image_handle();
    let Ok(mut fs) = uefi::boot::get_image_file_system(image) else {
        return false;
    };
    list_archive_from_fs(&mut fs, prefix, result)
}

fn try_list_archive_from_any_fs(prefix: &str, result: &mut Vec<FileEntry>) -> bool {
    let Ok(handles) = boot::find_handles::<uefi::proto::media::fs::SimpleFileSystem>() else {
        return false;
    };
    for handle in handles {
        let params = boot::OpenProtocolParams {
            handle,
            agent: boot::image_handle(),
            controller: None,
        };
        let Ok(mut fs) = (unsafe {
            boot::open_protocol::<uefi::proto::media::fs::SimpleFileSystem>(
                params,
                boot::OpenProtocolAttributes::GetProtocol,
            )
        }) else {
            continue;
        };
        if list_archive_from_fs(&mut fs, prefix, result) {
            return true;
        }
    }
    false
}

pub fn write_file(path: &str, data: &[u8]) -> bool {
    if let Some(mapped) = direct_fs_path(path) {
        if write_direct_file(&mapped, data) {
            return true;
        }
    }
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
    if let Some(mapped) = direct_fs_path(path) {
        if remove_direct_file(&mapped) {
            return;
        }
    }
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
    let _ = remove_direct_file(path);
}

fn remove_direct_file(path: &str) -> bool {
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
                    return true;
                }
            }
        }
    }
    false
}
