use uefi::proto::media::file::{File, FileAttribute, FileMode};
use uefi::CStr16;

struct EmbeddedFile {
    path: &'static [u8],
    data: &'static [u8],
}

static EMBEDDED_FILES: &[EmbeddedFile] = &[
    EmbeddedFile { path: b"apps/blank.warp", data: include_bytes!("app/blank.warp") },
    EmbeddedFile { path: b"apps/settings.warp", data: include_bytes!("app/settings.warp") },
    EmbeddedFile { path: b"apps/warpdemo.warp", data: include_bytes!("app/warpdemo.warp") },
    EmbeddedFile { path: b"apps/demo.u1", data: include_bytes!("app/demo.u1") },
    EmbeddedFile { path: b"apps/task.warp", data: include_bytes!("app/task.warp") },
    EmbeddedFile { path: b"apps/note.warp", data: include_bytes!("app/note.warp") },
    EmbeddedFile { path: b"apps/files.warp", data: include_bytes!("app/files.warp") },
    EmbeddedFile { path: b"app/icon/settings.png", data: include_bytes!("app/icon/settings.png") },
    EmbeddedFile { path: b"app/icon/note.png", data: include_bytes!("app/icon/note.png") },
    EmbeddedFile { path: b"app/icon/noname.png", data: include_bytes!("app/icon/noname.png") },
    EmbeddedFile { path: b"app/icon/manager.png", data: include_bytes!("app/icon/manager.png") },
    EmbeddedFile { path: b"app/icon/files.png", data: include_bytes!("app/icon/files.png") },
];

fn read_embedded(path: &str) -> Option<&'static [u8]> {
    for f in EMBEDDED_FILES {
        if core::str::from_utf8(f.path).ok() == Some(path) {
            return Some(f.data);
        }
    }
    None
}

pub fn read_file(path: &str) -> alloc::vec::Vec<u8> {
    if let Some(data) = read_embedded(path) {
        return alloc::vec::Vec::from(data);
    }

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
    let mut buf = [0u16; 256];
    let mut i = 0;
    for ch in path.encode_utf16() {
        if i + 1 < buf.len() {
            buf[i] = ch;
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
