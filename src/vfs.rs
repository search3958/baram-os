use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::CStr16;

pub fn read_file(path: &str) -> alloc::vec::Vec<u8> {
    let ih = uefi::boot::image_handle();
    let fs_result = uefi::boot::get_image_file_system(ih);
    let mut fs = match fs_result {
        Ok(f) => f,
        Err(_) => return alloc::vec::Vec::new(),
    };
    let mut root = match fs.open_volume() {
        Ok(r) => r,
        Err(_) => return alloc::vec::Vec::new(),
    };

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
    let cpath = match CStr16::from_u16_with_nul(&buf[..=i]) {
        Ok(c) => c,
        Err(_) => return alloc::vec::Vec::new(),
    };

    let handle = match root.open(cpath, FileMode::Read, FileAttribute::empty()) {
        Ok(h) => h,
        Err(_) => return alloc::vec::Vec::new(),
    };
    let mut file = match handle.into_regular_file() {
        Some(f) => f,
        None => return alloc::vec::Vec::new(),
    };
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
    contents
}

pub fn read_file_str(path: &str) -> alloc::string::String {
    let bytes = read_file(path);
    alloc::string::String::from_utf8(bytes).unwrap_or_default()
}
