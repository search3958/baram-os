#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;

pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

pub const ELFCLASS32: u8 = 1;
pub const ELFCLASS64: u8 = 2;

pub const ELFDATA2LSB: u8 = 1;
pub const ELFDATA2MSB: u8 = 2;

pub const ET_NONE: u16 = 0;
pub const ET_REL: u16 = 1;
pub const ET_EXEC: u16 = 2;
pub const ET_DYN: u16 = 3;
pub const ET_CORE: u16 = 4;

pub const EM_NONE: u16 = 0;
pub const EM_ARM: u16 = 40;
pub const EM_AARCH64: u16 = 183;

pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_NOTE: u32 = 4;
pub const PT_SHLIB: u32 = 5;
pub const PT_PHDR: u32 = 6;
pub const PT_TLS: u32 = 7;
pub const PT_GNU_EH_FRAME: u32 = 0x6474e550;
pub const PT_GNU_STACK: u32 = 0x6474e551;
pub const PT_GNU_RELRO: u32 = 0x6474e552;

pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_RELA: u32 = 4;
pub const SHT_HASH: u32 = 5;
pub const SHT_DYNAMIC: u32 = 6;
pub const SHT_NOTE: u32 = 7;
pub const SHT_NOBITS: u32 = 8;
pub const SHT_REL: u32 = 9;
pub const SHT_DYNSYM: u32 = 11;

pub const DT_NULL: u64 = 0;
pub const DT_NEEDED: u64 = 1;
pub const DT_PLTRELSZ: u64 = 2;
pub const DT_PLTGOT: u64 = 3;
pub const DT_HASH: u64 = 4;
pub const DT_STRTAB: u64 = 5;
pub const DT_SYMTAB: u64 = 6;
pub const DT_RELA: u64 = 7;
pub const DT_RELASZ: u64 = 8;
pub const DT_RELAENT: u64 = 9;
pub const DT_STRSZ: u64 = 10;
pub const DT_SYMENT: u64 = 11;
pub const DT_INIT: u64 = 12;
pub const DT_FINI: u64 = 13;
pub const DT_SONAME: u64 = 14;
pub const DT_RPATH: u64 = 15;
pub const DT_SYMBOLIC: u64 = 16;
pub const DT_REL: u64 = 17;
pub const DT_RELSZ: u64 = 18;
pub const DT_RELENT: u64 = 19;
pub const DT_PLTREL: u64 = 20;
pub const DT_DEBUG: u64 = 21;
pub const DT_TEXTREL: u64 = 22;
pub const DT_JMPREL: u64 = 23;
pub const DT_BIND_NOW: u64 = 24;
pub const DT_INIT_ARRAY: u64 = 25;
pub const DT_FINI_ARRAY: u64 = 26;
pub const DT_INIT_ARRAYSZ: u64 = 27;
pub const DT_FINI_ARRAYSZ: u64 = 28;

pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;
pub const STB_WEAK: u8 = 2;

pub const STT_NOTYPE: u8 = 0;
pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;
pub const STT_SECTION: u8 = 3;
pub const STT_FILE: u8 = 4;

pub const STV_DEFAULT: u8 = 0;
pub const STV_INTERNAL: u8 = 1;
pub const STV_HIDDEN: u8 = 2;
pub const STV_PROTECTED: u8 = 3;

pub const SHN_UNDEF: u16 = 0;
pub const SHN_ABS: u16 = 0xFFF1;
pub const SHN_COMMON: u16 = 0xFFF2;
pub const STN_UNDEF: u32 = 0;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

impl Elf64Header {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < core::mem::size_of::<Elf64Header>() {
            return None;
        }

        let header = unsafe { &*(data.as_ptr() as *const Elf64Header) };

        if header.e_ident[..4] != ELF_MAGIC {
            return None;
        }

        if header.e_ident[4] != ELFCLASS64 {
            return None;
        }

        if header.e_ident[5] != ELFDATA2LSB {
            return None;
        }

        if header.e_machine != EM_AARCH64 {
            return None;
        }

        Some(*header)
    }

    pub fn is_executable(&self) -> bool {
        self.e_type == ET_EXEC
    }

    pub fn is_shared_object(&self) -> bool {
        self.e_type == ET_DYN
    }

    pub fn is_pie(&self) -> bool {
        self.e_type == ET_DYN && self.e_entry != 0
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Elf64ProgramHeader {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

impl Elf64ProgramHeader {
    pub fn is_loadable(&self) -> bool {
        self.p_type == PT_LOAD
    }

    pub fn is_executable(&self) -> bool {
        self.p_flags & PF_X != 0
    }

    pub fn is_writable(&self) -> bool {
        self.p_flags & PF_W != 0
    }

    pub fn is_readable(&self) -> bool {
        self.p_flags & PF_R != 0
    }

    pub fn is_dynamic(&self) -> bool {
        self.p_type == PT_DYNAMIC
    }

    pub fn file_size(&self) -> usize {
        self.p_filesz as usize
    }

    pub fn mem_size(&self) -> usize {
        self.p_memsz as usize
    }

    pub fn vaddr(&self) -> usize {
        self.p_vaddr as usize
    }

    pub fn offset(&self) -> usize {
        self.p_offset as usize
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Elf64SectionHeader {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u64,
    pub sh_entsize: u64,
}

impl Elf64SectionHeader {
    pub fn is_valid(&self) -> bool {
        self.sh_type != SHT_NULL
    }

    pub fn name_offset(&self) -> u32 {
        self.sh_name
    }

    pub fn section_type(&self) -> u32 {
        self.sh_type
    }

    pub fn is_allocatable(&self) -> bool {
        self.sh_flags & 0x2 != 0
    }

    pub fn addr(&self) -> usize {
        self.sh_addr as usize
    }

    pub fn offset(&self) -> usize {
        self.sh_offset as usize
    }

    pub fn size(&self) -> usize {
        self.sh_size as usize
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Elf64Dynamic {
    pub d_tag: u64,
    pub d_val: u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

impl Elf64Sym {
    pub fn bind(&self) -> u8 {
        self.st_info >> 4
    }

    pub fn r#type(&self) -> u8 {
        self.st_info & 0xF
    }

    pub fn visibility(&self) -> u8 {
        self.st_other & 0x3
    }

    pub fn is_defined(&self) -> bool {
        self.st_shndx != SHN_UNDEF && self.st_shndx != SHN_COMMON
    }

    pub fn is_import(&self) -> bool {
        self.st_shndx == SHN_UNDEF
    }

    pub fn is_global(&self) -> bool {
        self.bind() == STB_GLOBAL
    }

    pub fn is_weak(&self) -> bool {
        self.bind() == STB_WEAK
    }

    pub fn is_function(&self) -> bool {
        self.r#type() == STT_FUNC
    }

    pub fn is_object(&self) -> bool {
        self.r#type() == STT_OBJECT
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Elf64Rela {
    pub r_offset: u64,
    pub r_info: u64,
    pub r_addend: i64,
}

impl Elf64Rela {
    pub fn sym(&self) -> usize {
        (self.r_info >> 32) as usize
    }

    pub fn r#type(&self) -> u32 {
        (self.r_info & 0xFFFFFFFF) as u32
    }

    pub fn addend(&self) -> i64 {
        self.r_addend
    }
}

pub const R_AARCH64_NONE: u32 = 0;
pub const R_AARCH64_ABS64: u32 = 257;
pub const R_AARCH64_GLOB_DAT: u32 = 1025;
pub const R_AARCH64_JUMP_SLOT: u32 = 1026;
pub const R_AARCH64_RELATIVE: u32 = 1027;
pub const R_AARCH64_TLS_TPREL64: u32 = 1030;
pub const R_AARCH64_TLS_DTPMOD64: u32 = 1028;
pub const R_AARCH64_TLS_DTPREL64: u32 = 1029;

pub struct ElfFile {
    pub header: Elf64Header,
    pub program_headers: Vec<Elf64ProgramHeader>,
    pub section_headers: Vec<Elf64SectionHeader>,
    pub data: Vec<u8>,
}

impl ElfFile {
    pub fn parse(data: Vec<u8>) -> Option<Self> {
        let header = Elf64Header::parse(&data)?;

        let mut program_headers = Vec::new();
        for i in 0..header.e_phnum {
            let offset = header.e_phoff as usize + i as usize * header.e_phentsize as usize;
            if offset + header.e_phentsize as usize > data.len() {
                return None;
            }
            let ph = unsafe { &*(data[offset..].as_ptr() as *const Elf64ProgramHeader) };
            program_headers.push(*ph);
        }

        let mut section_headers = Vec::new();
        for i in 0..header.e_shnum {
            let offset = header.e_shoff as usize + i as usize * header.e_shentsize as usize;
            if offset + header.e_shentsize as usize > data.len() {
                return None;
            }
            let sh = unsafe { &*(data[offset..].as_ptr() as *const Elf64SectionHeader) };
            section_headers.push(*sh);
        }

        Some(Self {
            header,
            program_headers,
            section_headers,
            data,
        })
    }

    pub fn get_string(&self, offset: usize) -> &str {
        if offset >= self.data.len() {
            return "";
        }
        let start = offset;
        let mut end = offset;
        while end < self.data.len() && self.data[end] != 0 {
            end += 1;
        }
        core::str::from_utf8(&self.data[start..end]).unwrap_or("")
    }

    pub fn get_dynamic(&self) -> Option<&[Elf64Dynamic]> {
        let ph = self.program_headers.iter().find(|p| p.is_dynamic())?;
        let offset = ph.offset();
        let size = ph.file_size();
        if offset + size > self.data.len() {
            return None;
        }
        let count = size / core::mem::size_of::<Elf64Dynamic>();
        Some(unsafe {
            core::slice::from_raw_parts(
                self.data[offset..].as_ptr() as *const Elf64Dynamic,
                count,
            )
        })
    }

    pub fn get_symbols(&self, section_type: u32) -> Option<&[Elf64Sym]> {
        let sh = self.section_headers.iter().find(|s| s.section_type() == section_type)?;
        let offset = sh.offset();
        let size = sh.size();
        if offset + size > self.data.len() {
            return None;
        }
        let count = size / core::mem::size_of::<Elf64Sym>();
        Some(unsafe {
            core::slice::from_raw_parts(
                self.data[offset..].as_ptr() as *const Elf64Sym,
                count,
            )
        })
    }

    pub fn get_relocations(&self) -> Option<&[Elf64Rela]> {
        let sh = self.section_headers.iter().find(|s| s.section_type() == SHT_RELA)?;
        let offset = sh.offset();
        let size = sh.size();
        if offset + size > self.data.len() {
            return None;
        }
        let count = size / core::mem::size_of::<Elf64Rela>();
        Some(unsafe {
            core::slice::from_raw_parts(
                self.data[offset..].as_ptr() as *const Elf64Rela,
                count,
            )
        })
    }

    pub fn get_string_table(&self) -> Option<&[u8]> {
        let sh = self.section_headers.iter().find(|s| s.section_type() == SHT_STRTAB)?;
        let offset = sh.offset();
        let size = sh.size();
        if offset + size > self.data.len() {
            return None;
        }
        Some(&self.data[offset..offset + size])
    }

    pub fn calculate_load_size(&self) -> usize {
        let mut max_addr = 0;
        let mut min_addr = usize::MAX;

        for ph in &self.program_headers {
            if ph.is_loadable() {
                let vaddr = ph.vaddr();
                let end = vaddr + ph.mem_size();
                if vaddr < min_addr {
                    min_addr = vaddr;
                }
                if end > max_addr {
                    max_addr = end;
                }
            }
        }

        if max_addr > min_addr {
            max_addr - min_addr
        } else {
            0
        }
    }

    pub fn get_load_segments(&self) -> Vec<&Elf64ProgramHeader> {
        self.program_headers.iter()
            .filter(|p| p.is_loadable())
            .collect()
    }
}
