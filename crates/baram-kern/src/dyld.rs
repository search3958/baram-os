#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::collections::BTreeMap;
use crate::elf::{
    ElfFile,
    DT_NEEDED,
    DT_INIT, DT_FINI, DT_SONAME,
    STB_GLOBAL, STB_WEAK,
};
use crate::vmm::{VirtualMemoryManager, PAGE_SIZE};

pub const LIBRARY_SEARCH_PATHS: &[&str] = &[
    "/lib",
    "/usr/lib",
    "/system/lib",
];

pub struct DynamicLinker {
    libraries: Vec<LoadedLibrary>,
    global_symbols: BTreeMap<String, SymbolEntry>,
    symbol_table_addr: usize,
    string_table_addr: usize,
    next_load_addr: usize,
    next_id: usize,
}

#[derive(Clone)]
pub struct SymbolEntry {
    pub name: String,
    pub value: usize,
    pub size: usize,
    pub library_id: usize,
    pub bind: u8,
    pub visibility: u8,
}

impl SymbolEntry {
    pub fn is_defined(&self) -> bool {
        self.value != 0
    }

    pub fn is_global(&self) -> bool {
        self.bind == STB_GLOBAL
    }

    pub fn is_weak(&self) -> bool {
        self.bind == STB_WEAK
    }
}

#[derive(Clone)]
pub struct LoadedLibrary {
    pub id: usize,
    pub name: String,
    pub soname: String,
    pub base_addr: usize,
    pub load_addr: usize,
    pub text_addr: usize,
    pub text_size: usize,
    pub data_addr: usize,
    pub data_size: usize,
    pub bss_addr: usize,
    pub bss_size: usize,
    pub init_addr: usize,
    pub fini_addr: usize,
    pub symbols: Vec<SymbolEntry>,
    pub needed: Vec<String>,
    pub is_loaded: bool,
    pub is_initialized: bool,
    pub ref_count: u32,
}

impl DynamicLinker {
    pub fn new() -> Self {
        Self {
            libraries: Vec::new(),
            global_symbols: BTreeMap::new(),
            symbol_table_addr: 0,
            string_table_addr: 0,
            next_load_addr: 0x7F00_0000_0000_0000,
            next_id: 0,
        }
    }

    pub fn load_library(&mut self, name: &str, vmm: &mut VirtualMemoryManager, ttbr: usize) -> Result<usize, LinkError> {
        if let Some(id) = self.find_library(name) {
            self.libraries[id].ref_count += 1;
            return Ok(id);
        }

        let data = self.read_library(name)?;
        let elf = ElfFile::parse(data).ok_or(LinkError::InvalidElf)?;

        if !elf.header.is_shared_object() && !elf.header.is_executable() {
            return Err(LinkError::InvalidElfType);
        }

        let load_size = elf.calculate_load_size();
        if load_size == 0 {
            return Err(LinkError::InvalidSegments);
        }

        let load_addr = self.allocate_address(load_size);

        let id = self.next_id;
        self.next_id += 1;

        let mut library = LoadedLibrary {
            id,
            name: name.to_string(),
            soname: String::new(),
            base_addr: load_addr,
            load_addr,
            text_addr: 0,
            text_size: 0,
            data_addr: 0,
            data_size: 0,
            bss_addr: 0,
            bss_size: 0,
            init_addr: 0,
            fini_addr: 0,
            symbols: Vec::new(),
            needed: Vec::new(),
            is_loaded: false,
            is_initialized: false,
            ref_count: 1,
        };

        self.load_segments(&elf, &mut library, vmm, ttbr)?;
        self.load_dynamic_info(&elf, &mut library);
        self.load_symbols(&elf, &mut library);

        library.is_loaded = true;

        for needed in &library.needed {
            self.load_library(needed, vmm, ttbr)?;
        }

        self.libraries.push(library);

        Ok(id)
    }

    pub fn resolve_symbol(&self, name: &str) -> Option<usize> {
        if let Some(entry) = self.global_symbols.get(name) {
            return Some(entry.value);
        }

        for library in &self.libraries {
            if !library.is_loaded {
                continue;
            }
            for symbol in &library.symbols {
                if symbol.name == name && symbol.bind == STB_GLOBAL {
                    return Some(symbol.value);
                }
            }
        }

        None
    }

    pub fn apply_relocations(&mut self, library_id: usize, vmm: &VirtualMemoryManager, ttbr: usize) -> Result<(), LinkError> {
        if library_id >= self.libraries.len() {
            return Err(LinkError::LibraryNotFound);
        }

        let library = &self.libraries[library_id];
        let base = library.base_addr;

        self.apply_rel_relocations(library_id, base, vmm, ttbr)?;
        self.apply_rela_relocations(library_id, base, vmm, ttbr)?;

        Ok(())
    }

    pub fn initialize_libraries(&mut self) -> Result<(), LinkError> {
        for library in &mut self.libraries {
            if library.is_loaded && !library.is_initialized {
                if library.init_addr != 0 {
                    let init_fn: extern "C" fn() = unsafe {
                        core::mem::transmute(library.init_addr)
                    };
                    init_fn();
                }
                library.is_initialized = true;
            }
        }

        Ok(())
    }

    pub fn finalize_libraries(&mut self) -> Result<(), LinkError> {
        for library in self.libraries.iter_mut().rev() {
            if library.is_initialized && library.ref_count == 0 {
                if library.fini_addr != 0 {
                    let fini_fn: extern "C" fn() = unsafe {
                        core::mem::transmute(library.fini_addr)
                    };
                    fini_fn();
                }
                library.is_initialized = false;
            }
        }

        Ok(())
    }

    pub fn unload_library(&mut self, library_id: usize) -> Result<(), LinkError> {
        if library_id >= self.libraries.len() {
            return Err(LinkError::LibraryNotFound);
        }

        let should_finalize = {
            let library = &self.libraries[library_id];
            library.is_loaded && library.ref_count <= 1
        };

        if should_finalize {
            let library = &self.libraries[library_id];
            if library.is_initialized && library.fini_addr != 0 {
                let fini_fn: extern "C" fn() = unsafe {
                    core::mem::transmute(library.fini_addr)
                };
                fini_fn();
            }
        }

        self.remove_global_symbols(library_id);

        let library = &mut self.libraries[library_id];
        library.ref_count = library.ref_count.saturating_sub(1);
        if library.ref_count == 0 {
            library.is_initialized = false;
            library.is_loaded = false;
        }

        Ok(())
    }

    pub fn get_library(&self, library_id: usize) -> Option<&LoadedLibrary> {
        self.libraries.get(library_id)
    }

    pub fn get_library_mut(&mut self, library_id: usize) -> Option<&mut LoadedLibrary> {
        self.libraries.get_mut(library_id)
    }

    pub fn get_libraries(&self) -> &[LoadedLibrary] {
        &self.libraries
    }

    fn find_library(&self, name: &str) -> Option<usize> {
        for (i, library) in self.libraries.iter().enumerate() {
            if library.name == name || library.soname == name {
                return Some(i);
            }
        }
        None
    }

    fn read_library(&self, name: &str) -> Result<Vec<u8>, LinkError> {
        for path in LIBRARY_SEARCH_PATHS {
            let full_path = alloc::format!("{}/{}", path, name);
            if let Some(data) = self.read_file_from_fs(&full_path) {
                return Ok(data);
            }
        }

        Err(LinkError::LibraryNotFound)
    }

    fn read_file_from_fs(&self, path: &str) -> Option<Vec<u8>> {
        

        let data = crate::loader::read_file(path)?;
        Some(data)
    }

    fn allocate_address(&mut self, size: usize) -> usize {
        let aligned_size = (size + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let addr = self.next_load_addr;
        self.next_load_addr += aligned_size;
        addr
    }

    fn load_segments(&mut self, elf: &ElfFile, library: &mut LoadedLibrary, vmm: &mut VirtualMemoryManager, ttbr: usize) -> Result<(), LinkError> {
        let segments = elf.get_load_segments();
        if segments.is_empty() {
            return Err(LinkError::InvalidSegments);
        }

        let base = library.base_addr;
        let mut text_end = 0;
        let mut data_start = 0;
        let mut data_end = 0;
        let mut bss_start = 0;
        let mut bss_end = 0;

        for (_i, segment) in segments.iter().enumerate() {
            let vaddr = segment.vaddr() + base;
            let mem_size = segment.mem_size();
            let file_size = segment.file_size();
            let flags = segment.p_flags;

            if flags & 0x1 != 0 {
                library.text_addr = vaddr;
                library.text_size = mem_size;
                text_end = vaddr + mem_size;
            } else if flags & 0x2 != 0 {
                if data_start == 0 {
                    data_start = vaddr;
                }
                data_end = vaddr + mem_size;
                library.data_addr = vaddr;
                library.data_size = mem_size;
            }

            let page_vaddr = vaddr & !(PAGE_SIZE - 1);
            let page_count = (mem_size + PAGE_SIZE - 1) / PAGE_SIZE;

            let mut page_attrs = 0u64;
            if flags & 0x1 != 0 {
                page_attrs |= 0x10; // XN disabled for executable
            } else {
                page_attrs |= 0x80; // XN enabled for non-executable
            }
            if flags & 0x2 != 0 {
                page_attrs |= 0x0; // RW
            } else {
                page_attrs |= 0x800; // RO
            }
            page_attrs |= 0x400; // AF (Access Flag)
            page_attrs |= 0x300; // SH (Inner Shareable)

            for page in 0..page_count {
                let virt = page_vaddr + page * PAGE_SIZE;
                let phys = self.allocate_physical_page();

                if !vmm.map_page(ttbr, virt, phys, page_attrs) {
                    return Err(LinkError::MappingFailed);
                }

                if page * PAGE_SIZE < file_size {
                    let copy_size = core::cmp::min(PAGE_SIZE, file_size - page * PAGE_SIZE);
                    let src_offset = segment.offset() + page * PAGE_SIZE;
                    if src_offset + copy_size <= elf.data.len() {
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                elf.data[src_offset..].as_ptr(),
                                phys as *mut u8,
                                copy_size,
                            );
                        }
                    }
                }
            }

            if mem_size > file_size {
                let bss_vaddr = vaddr + file_size;
                let bss_size = mem_size - file_size;
                if bss_start == 0 {
                    bss_start = bss_vaddr;
                }
                bss_end = bss_vaddr + bss_size;
                library.bss_addr = bss_vaddr;
                library.bss_size = bss_size;
            }
        }

        Ok(())
    }

    fn load_dynamic_info(&self, elf: &ElfFile, library: &mut LoadedLibrary) {
        if let Some(dynamic) = elf.get_dynamic() {
            for entry in dynamic {
                match entry.d_tag {
                    DT_NEEDED => {
                        if let Some(strtab) = elf.get_string_table() {
                            let offset = entry.d_val as usize;
                            if offset < strtab.len() {
                                let mut end = offset;
                                while end < strtab.len() && strtab[end] != 0 {
                                    end += 1;
                                }
                                if let Ok(name) = core::str::from_utf8(&strtab[offset..end]) {
                                    library.needed.push(name.to_string());
                                }
                            }
                        }
                    }
                    DT_SONAME => {
                        if let Some(strtab) = elf.get_string_table() {
                            let offset = entry.d_val as usize;
                            if offset < strtab.len() {
                                let mut end = offset;
                                while end < strtab.len() && strtab[end] != 0 {
                                    end += 1;
                                }
                                if let Ok(name) = core::str::from_utf8(&strtab[offset..end]) {
                                    library.soname = name.to_string();
                                }
                            }
                        }
                    }
                    DT_INIT => {
                        library.init_addr = entry.d_val as usize + library.base_addr;
                    }
                    DT_FINI => {
                        library.fini_addr = entry.d_val as usize + library.base_addr;
                    }
                    _ => {}
                }
            }
        }
    }

    fn load_symbols(&mut self, elf: &ElfFile, library: &mut LoadedLibrary) {
        if let Some(syms) = elf.get_symbols(crate::elf::SHT_DYNSYM) {
            if let Some(strtab) = elf.get_string_table() {
                for sym in syms {
                    if sym.st_name as usize >= strtab.len() {
                        continue;
                    }

                    let mut end = sym.st_name as usize;
                    while end < strtab.len() && strtab[end] != 0 {
                        end += 1;
                    }

                    if let Ok(name) = core::str::from_utf8(&strtab[sym.st_name as usize..end]) {
                        let value = if sym.is_defined() {
                            sym.st_value as usize + library.base_addr
                        } else {
                            0
                        };

                        let entry = SymbolEntry {
                            name: name.to_string(),
                            value,
                            size: sym.st_size as usize,
                            library_id: library.id,
                            bind: sym.bind(),
                            visibility: sym.visibility(),
                        };

                        if sym.is_global() && !name.is_empty() {
                            self.global_symbols.insert(name.to_string(), entry.clone());
                        }

                        library.symbols.push(entry);
                    }
                }
            }
        }
    }

    fn apply_rel_relocations(&self, library_id: usize, _base: usize, vmm: &VirtualMemoryManager, ttbr: usize) -> Result<(), LinkError> {
        let library = &self.libraries[library_id];
        let base = library.base_addr;

        for sym in &library.symbols {
            if !sym.is_defined() || sym.value == 0 {
                continue;
            }

            let addr = sym.value;
            let value = base + sym.value;

            self.write_relocation(addr, value, vmm, ttbr)?;
        }

        Ok(())
    }

    fn apply_rela_relocations(&self, library_id: usize, _base: usize, vmm: &VirtualMemoryManager, ttbr: usize) -> Result<(), LinkError> {
        let library = &self.libraries[library_id];

        for sym in &library.symbols {
            if !sym.is_defined() || sym.value == 0 {
                continue;
            }

            let reloc_addr = sym.value;
            let sym_value = sym.value;

            match sym.bind {
                STB_GLOBAL => {
                    if let Some(resolved) = self.resolve_symbol(&sym.name) {
                        self.write_relocation(reloc_addr, resolved, vmm, ttbr)?;
                    } else {
                        self.write_relocation(reloc_addr, sym_value, vmm, ttbr)?;
                    }
                }
                STB_WEAK => {
                    if let Some(resolved) = self.resolve_symbol(&sym.name) {
                        self.write_relocation(reloc_addr, resolved, vmm, ttbr)?;
                    } else {
                        self.write_relocation(reloc_addr, 0, vmm, ttbr)?;
                    }
                }
                _ => {
                    self.write_relocation(reloc_addr, sym_value, vmm, ttbr)?;
                }
            }
        }

        Ok(())
    }

    fn write_relocation(&self, addr: usize, value: usize, vmm: &VirtualMemoryManager, ttbr: usize) -> Result<(), LinkError> {
        if let Some(phys_addr) = vmm.translate(ttbr, addr) {
            unsafe {
                core::ptr::write_volatile(phys_addr as *mut usize, value);
            }
            Ok(())
        } else {
            Err(LinkError::MappingFailed)
        }
    }

    fn remove_global_symbols(&mut self, library_id: usize) {
        let to_remove: Vec<String> = self.global_symbols.iter()
            .filter(|(_, entry)| entry.library_id == library_id)
            .map(|(name, _)| name.clone())
            .collect();

        for name in to_remove {
            self.global_symbols.remove(&name);
        }
    }

    fn allocate_physical_page(&self) -> usize {
        

        let layout = core::alloc::Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap();
        unsafe {
            let ptr = alloc::alloc::alloc_zeroed(layout);
            if ptr.is_null() {
                0
            } else {
                ptr as usize
            }
        }
    }
}

#[derive(Debug)]
pub enum LinkError {
    InvalidElf,
    InvalidElfType,
    InvalidSegments,
    LibraryNotFound,
    SymbolNotFound(String),
    MappingFailed,
    RelocationFailed,
    CircularDependency,
    OutOfMemory,
}
