#![no_std]

extern crate alloc;


pub const PE_MZ: u16 = 0x5A4D;
pub const PE_SIGNATURE: u32 = 0x00004550;
pub const IMAGE_DIRECTORY_ENTRY_BASERELOC: usize = 5;
pub const IMAGE_REL_BASED_DIR64: u16 = 10;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct DosHeader {
    e_magic: u16,
    _reserved: [u16; 29],
    e_lfanew: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct CoffHeader {
    machine: u16,
    number_of_sections: u16,
    time_date_stamp: u32,
    pointer_to_symbol_table: u32,
    number_of_symbols: u32,
    size_of_optional_header: u16,
    characteristics: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct OptionalHeader64 {
    magic: u16,
    major_linker_version: u8,
    minor_linker_version: u8,
    size_of_code: u32,
    size_of_initialized_data: u32,
    size_of_uninitialized_data: u32,
    address_of_entry_point: u32,
    base_of_code: u32,
    image_base: u64,
    section_alignment: u32,
    file_alignment: u32,
    major_os_version: u16,
    minor_os_version: u16,
    major_image_version: u16,
    minor_image_version: u16,
    major_subsystem_version: u16,
    minor_subsystem_version: u16,
    win32_version_value: u32,
    size_of_image: u32,
    size_of_headers: u32,
    checksum: u32,
    subsystem: u16,
    dll_characteristics: u16,
    size_of_stack_reserve: u64,
    size_of_stack_commit: u64,
    size_of_heap_reserve: u64,
    size_of_heap_commit: u64,
    loader_flags: u32,
    number_of_rva_and_sizes: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct DataDirectory {
    virtual_address: u32,
    size: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct SectionHeader {
    name: [u8; 8],
    virtual_size: u32,
    virtual_address: u32,
    size_of_raw_data: u32,
    pointer_to_raw_data: u32,
    pointer_to_relocations: u32,
    pointer_to_linenumbers: u32,
    number_of_relocations: u16,
    number_of_linenumbers: u16,
    characteristics: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct BaseRelocationBlock {
    virtual_address: u32,
    size_of_block: u32,
}

pub struct LoadedModule {
    pub base_addr: *mut u8,
    pub size: usize,
    pub entry_point: usize,
    pub exports: *const super::subsystem::SubsystemExports,
}

unsafe impl Send for LoadedModule {}
unsafe impl Sync for LoadedModule {}

pub fn load_pe_from_memory(data: &[u8]) -> Result<LoadedModule, LoadError> {
    if data.len() < core::mem::size_of::<DosHeader>() {
        return Err(LoadError::InvalidFormat);
    }

    let dos_header = unsafe { &*(data.as_ptr() as *const DosHeader) };
    if dos_header.e_magic != PE_MZ {
        return Err(LoadError::InvalidFormat);
    }

    let pe_offset = dos_header.e_lfanew as usize;
    if pe_offset + 4 + core::mem::size_of::<CoffHeader>() > data.len() {
        return Err(LoadError::InvalidFormat);
    }

    let pe_sig = unsafe { core::ptr::read_unaligned(data[pe_offset..].as_ptr() as *const u32) };
    if pe_sig != PE_SIGNATURE {
        return Err(LoadError::InvalidFormat);
    }

    let coff_offset = pe_offset + 4;
    let coff = unsafe { &*(data[coff_offset..].as_ptr() as *const CoffHeader) };

    let opt_offset = coff_offset + core::mem::size_of::<CoffHeader>();
    let magic = unsafe { core::ptr::read_unaligned(data[opt_offset..].as_ptr() as *const u16) };

    let (entry_rva, image_base, size_of_image, _section_alignment, data_dirs_offset, num_data_dirs) = if magic == 0x20b {
        let opt = unsafe { &*(data[opt_offset..].as_ptr() as *const OptionalHeader64) };
        let dd_off = opt_offset + core::mem::size_of::<OptionalHeader64>();
        (opt.address_of_entry_point, opt.image_base, opt.size_of_image, opt.section_alignment, dd_off, opt.number_of_rva_and_sizes as usize)
    } else {
        return Err(LoadError::UnsupportedPEFormat);
    };

    let sections_offset = opt_offset + coff.size_of_optional_header as usize;

    let pages = (size_of_image as usize + 0xFFF) / 0x1000;
    let image = match uefi::boot::allocate_pages(
        uefi::boot::AllocateType::Address(image_base),
        uefi::boot::MemoryType::LOADER_DATA,
        pages,
    ) {
        Ok(ptr) => ptr.as_ptr(),
        Err(_) => {
            match uefi::boot::allocate_pages(
                uefi::boot::AllocateType::AnyPages,
                uefi::boot::MemoryType::LOADER_DATA,
                pages,
            ) {
                Ok(ptr) => ptr.as_ptr(),
                Err(_) => return Err(LoadError::AllocationFailed),
            }
        }
    };

    let headers_size = sections_offset + coff.number_of_sections as usize * core::mem::size_of::<SectionHeader>();
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), image, headers_size.min(data.len()));
    }

    for i in 0..coff.number_of_sections {
        let sec_off = sections_offset + i as usize * core::mem::size_of::<SectionHeader>();
        let sec = unsafe { &*(data[sec_off..].as_ptr() as *const SectionHeader) };

        if sec.size_of_raw_data == 0 && sec.virtual_size == 0 {
            continue;
        }

        let sec_size = sec.virtual_size.max(sec.size_of_raw_data) as usize;
        let dest = unsafe { image.add(sec.virtual_address as usize) };

        let file_size = sec.size_of_raw_data as usize;
        let src_off = sec.pointer_to_raw_data as usize;
        if src_off + file_size <= data.len() {
            unsafe {
                core::ptr::copy_nonoverlapping(data[src_off..].as_ptr(), dest, file_size);
                if sec_size > file_size {
                    core::ptr::write_bytes(dest.add(file_size), 0, sec_size - file_size);
                }
            }
        } else {
            unsafe {
                core::ptr::write_bytes(dest, 0, sec_size);
            }
        }
    }

    if num_data_dirs > IMAGE_DIRECTORY_ENTRY_BASERELOC {
        let reloc_dir_off = data_dirs_offset + IMAGE_DIRECTORY_ENTRY_BASERELOC * core::mem::size_of::<DataDirectory>();
        let reloc_dir = unsafe { &*(data[reloc_dir_off..].as_ptr() as *const DataDirectory) };

        if reloc_dir.virtual_address != 0 && reloc_dir.size != 0 {
            let delta = image as isize - image_base as isize;
            let mut block_offset = reloc_dir.virtual_address as usize;

            loop {
                if block_offset + core::mem::size_of::<BaseRelocationBlock>() > size_of_image as usize {
                    break;
                }
                let block = unsafe { &*(image.add(block_offset) as *const BaseRelocationBlock) };
                if block.size_of_block < 8 || block.size_of_block > 0x100000 {
                    break;
                }

                let num_entries = (block.size_of_block as usize - 8) / 2;
                for j in 0..num_entries {
                    let entry_off = block_offset + 8 + j * 2;
                    let entry = unsafe { core::ptr::read_unaligned(image.add(entry_off) as *const u16) };
                    let rel_type = entry >> 12;
                    let rel_offset = (entry & 0x0FFF) as usize;

                    if rel_type == IMAGE_REL_BASED_DIR64 {
                        let target = unsafe { image.add(block.virtual_address as usize + rel_offset) as *mut u64 };
                        unsafe {
                            let val = core::ptr::read_unaligned(target as *const u64);
                            core::ptr::write_unaligned(target, (val as isize + delta) as u64);
                        }
                    }
                }

                let next = block_offset + block.size_of_block as usize;
                if next == block_offset { break; }
                block_offset = next;
            }
        }
    }

    let entry_addr = unsafe { image.add(entry_rva as usize) } as usize;

    Ok(LoadedModule {
        base_addr: image,
        size: size_of_image as usize,
        entry_point: entry_addr,
        exports: core::ptr::null(),
    })
}

pub fn find_exports(module: &LoadedModule) -> Option<*const super::subsystem::SubsystemExports> {
    let image = module.base_addr;

    let dos_header = unsafe { &*(image as *const DosHeader) };
    let pe_offset = dos_header.e_lfanew as usize;
    let coff_offset = pe_offset + 4;
    let _coff = unsafe { &*(image.add(coff_offset) as *const CoffHeader) };
    let opt_offset = coff_offset + core::mem::size_of::<CoffHeader>();
    let magic = unsafe { core::ptr::read_unaligned(image.add(opt_offset) as *const u16) };

    if magic != 0x20b {
        return None;
    }

    let opt = unsafe { &*(image.add(opt_offset) as *const OptionalHeader64) };
    let data_dirs_offset = opt_offset + core::mem::size_of::<OptionalHeader64>();

    if opt.number_of_rva_and_sizes <= 0 {
        return None;
    }

    let export_dir_rva = unsafe {
        let dd = &*(image.add(data_dirs_offset) as *const DataDirectory);
        dd.virtual_address
    };

    if export_dir_rva == 0 {
        return None;
    }

    let export_dir = unsafe { &*(image.add(export_dir_rva as usize) as *const ExportDir) };

    let num_names = export_dir.number_of_names as usize;
    let names_ptr = unsafe { image.add(export_dir.address_of_names as usize) as *const u32 };
    let ordinals_ptr = unsafe { image.add(export_dir.address_of_name_ordinals as usize) as *const u16 };
    let functions_ptr = unsafe { image.add(export_dir.address_of_functions as usize) as *const u32 };

    for i in 0..num_names {
        let name_rva = unsafe { core::ptr::read_unaligned(names_ptr.add(i)) };
        let name_ptr = unsafe { image.add(name_rva as usize) as *const u8 };
        let name = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(name_ptr, 32)) };

        if name.starts_with("BARAM_SUBSYSTEM_EXPORTS") {
            let ordinal = unsafe { *ordinals_ptr.add(i) };
            let func_rva = unsafe { *functions_ptr.add(ordinal as usize) };
            let func_ptr = unsafe { image.add(func_rva as usize) };
            return Some(func_ptr as *const super::subsystem::SubsystemExports);
        }
    }

    None
}

#[repr(C, packed)]
struct ExportDir {
    characteristics: u32,
    time_date_stamp: u32,
    major_version: u16,
    minor_version: u16,
    name: u32,
    base: u32,
    number_of_functions: u32,
    number_of_names: u32,
    address_of_functions: u32,
    address_of_names: u32,
    address_of_name_ordinals: u32,
}

#[derive(Debug)]
pub enum LoadError {
    InvalidFormat,
    UnsupportedPEFormat,
    AllocationFailed,
    SectionLoadFailed,
    RelocationFailed,
}

pub fn read_file(path: &str) -> Option<alloc::vec::Vec<u8>> {
    
    use alloc::vec;

    let mut data = vec![0u8; 4096];
    let mut offset = 0;

    loop {
        let bytes_read = crate::loader::read_file_chunk(path, offset, &mut data[offset..]);
        if bytes_read == 0 {
            break;
        }
        offset += bytes_read;
        if offset >= data.len() {
            data.resize(data.len() * 2, 0);
        }
    }

    if offset == 0 {
        None
    } else {
        data.truncate(offset);
        Some(data)
    }
}

fn read_file_chunk(path: &str, offset: usize, buf: &mut [u8]) -> usize {
    
    use uefi::proto::media::file::{File, FileAttribute, FileMode};
    use uefi::CStr16;

    let ih = uefi::boot::image_handle();
    let mut fs = match uefi::boot::get_image_file_system(ih) {
        Ok(fs) => fs,
        Err(_) => return 0,
    };

    let mut root = match fs.open_volume() {
        Ok(root) => root,
        Err(_) => return 0,
    };

    let mut path_buf = [0u16; 256];
    let mut i = 0;
    for ch in path.bytes() {
        let c = if ch == b'/' { b'\\' } else { ch } as u16;
        if i + 1 < path_buf.len() {
            path_buf[i] = c;
            i += 1;
        }
    }
    path_buf[i] = 0;

    let cpath = match CStr16::from_u16_with_nul(&path_buf[..=i]) {
        Ok(p) => p,
        Err(_) => return 0,
    };

    let handle = match root.open(cpath, FileMode::Read, FileAttribute::empty()) {
        Ok(h) => h,
        Err(_) => return 0,
    };

    let mut file = match handle.into_regular_file() {
        Some(f) => f,
        None => return 0,
    };

    if offset > 0 {
        let _ = file.set_position(offset as u64);
    }

    match file.read(buf) {
        Ok(n) => n,
        Err(_) => 0,
    }
}
