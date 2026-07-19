#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::boxed::Box;
use crate::elf::{ElfFile, Elf64ProgramHeader, PT_LOAD, PT_INTERP, PT_DYNAMIC, PT_TLS};
use crate::vmm::{VirtualMemoryManager, PAGE_SIZE, PageTableEntry};
use crate::dyld::{DynamicLinker, LinkError};
use crate::process::{Process, ProcessState, ProcessPriority, CpuContext, STACK_SIZE};

pub const USER_SPACE_START: usize = 0x0000_0000_0001_0000;
pub const USER_SPACE_END: usize = 0x0000_7FFF_FFFF_F000;
pub const USER_STACK_SIZE: usize = 0x100000;
pub const USER_HEAP_SIZE: usize = 0x100000;

pub struct ProcessLoader {
    dynamic_linker: DynamicLinker,
    next_pid: u32,
}

#[derive(Clone)]
pub struct LoadInfo {
    pub entry_point: usize,
    pub stack_pointer: usize,
    pub program_break: usize,
    pub tls_addr: usize,
    pub tls_size: usize,
    pub interp_path: String,
    pub dynamic_addr: usize,
    pub dynamic_size: usize,
}

impl ProcessLoader {
    pub fn new() -> Self {
        Self {
            dynamic_linker: DynamicLinker::new(),
            next_pid: 1,
        }
    }

    pub fn load_executable(&mut self, path: &str, vmm: &mut VirtualMemoryManager) -> Result<(Process, LoadInfo), LoadError> {
        let data = self.read_file(path)?;
        let elf = ElfFile::parse(data).ok_or(LoadError::InvalidElf)?;

        if !elf.header.is_executable() && !elf.header.is_shared_object() {
            return Err(LoadError::InvalidElfType);
        }

        let pid = self.next_pid;
        self.next_pid += 1;

        let ttbr = vmm.create_address_space();
        if ttbr == 0 {
            return Err(LoadError::AddressSpaceCreationFailed);
        }

        let mut load_info = LoadInfo {
            entry_point: elf.header.e_entry as usize,
            stack_pointer: 0,
            program_break: 0,
            tls_addr: 0,
            tls_size: 0,
            interp_path: String::new(),
            dynamic_addr: 0,
            dynamic_size: 0,
        };

        self.load_segments(&elf, &mut load_info, vmm, ttbr)?;

        self.load_interp(&elf, &mut load_info)?;

        self.load_dynamic(&elf, &mut load_info)?;

        self.load_tls(&elf, &mut load_info)?;

        let stack = self.create_user_stack(vmm, ttbr)?;
        load_info.stack_pointer = stack + USER_STACK_SIZE;

        let heap = self.create_user_heap(vmm, ttbr)?;
        load_info.program_break = heap + USER_HEAP_SIZE;

        if load_info.entry_point == 0 && !load_info.interp_path.is_empty() {
            load_info.entry_point = self.load_interp_executable(&load_info.interp_path, vmm, ttbr)?;
        }

        let mut process = Process::new(pid, path, ProcessPriority::Normal);
        process.state = ProcessState::Created;
        process.page_table = ttbr;
        process.context = CpuContext {
            x0: 0,
            x1: 0,
            x2: 0,
            x3: 0,
            x4: 0,
            x5: 0,
            x6: 0,
            x7: 0,
            x8: 0,
            x9: 0,
            x10: 0,
            x11: 0,
            x12: 0,
            x13: 0,
            x14: 0,
            x15: 0,
            x16: 0,
            x17: 0,
            x18: 0,
            x19: 0,
            x20: 0,
            x21: 0,
            x22: 0,
            x23: 0,
            x24: 0,
            x25: 0,
            x26: 0,
            x27: 0,
            x28: 0,
            x29: 0,
            lr: 0,
            sp: load_info.stack_pointer as u64,
            elr_el1: load_info.entry_point as u64,
            spsr_el1: 0x3C0, // EL0 with all interrupts unmasked
            tcr_el1: 0,
            ttbr0_el1: ttbr as u64,
            ttbr1_el1: 0,
        };

        Ok((process, load_info))
    }

    pub fn load_shared_library(&mut self, path: &str, vmm: &mut VirtualMemoryManager, ttbr: usize) -> Result<usize, LoadError> {
        self.dynamic_linker.load_library(path, vmm, ttbr)
            .map_err(|_| LoadError::LibraryLoadFailed)
    }

    pub fn resolve_symbol(&self, name: &str) -> Option<usize> {
        self.dynamic_linker.resolve_symbol(name)
    }

    pub fn get_dynamic_linker(&self) -> &DynamicLinker {
        &self.dynamic_linker
    }

    pub fn get_dynamic_linker_mut(&mut self) -> &mut DynamicLinker {
        &mut self.dynamic_linker
    }

    fn load_segments(&self, elf: &ElfFile, load_info: &mut LoadInfo, vmm: &mut VirtualMemoryManager, ttbr: usize) -> Result<(), LoadError> {
        for ph in &elf.program_headers {
            if ph.p_type != PT_LOAD {
                continue;
            }

            let vaddr = ph.vaddr();
            let mem_size = ph.mem_size();
            let file_size = ph.file_size();
            let flags = ph.p_flags;

            let page_vaddr = vaddr & !(PAGE_SIZE - 1);
            let page_count = (mem_size + PAGE_SIZE - 1) / PAGE_SIZE;

            let mut page_attrs = 0u64;
            if flags & 0x1 != 0 {
                page_attrs |= 0x10;
            } else {
                page_attrs |= 0x80;
            }
            if flags & 0x2 != 0 {
                page_attrs |= 0x0;
            } else {
                page_attrs |= 0x800;
            }
            page_attrs |= 0x400;
            page_attrs |= 0x300;

            for page in 0..page_count {
                let virt = page_vaddr + page * PAGE_SIZE;
                let phys = self.allocate_physical_page();

                if !vmm.map_page(ttbr, virt, phys, page_attrs) {
                    return Err(LoadError::MappingFailed);
                }

                if page * PAGE_SIZE < file_size {
                    let copy_size = core::cmp::min(PAGE_SIZE, file_size - page * PAGE_SIZE);
                    let src_offset = ph.offset() + page * PAGE_SIZE;
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
        }

        Ok(())
    }

    fn load_interp(&self, elf: &ElfFile, load_info: &mut LoadInfo) -> Result<(), LoadError> {
        for ph in &elf.program_headers {
            if ph.p_type == PT_INTERP {
                let offset = ph.offset();
                let size = ph.file_size();
                if offset + size <= elf.data.len() {
                    let interp = core::str::from_utf8(&elf.data[offset..offset + size])
                        .unwrap_or("");
                    load_info.interp_path = interp.trim_end_matches('\0').to_string();
                }
                break;
            }
        }
        Ok(())
    }

    fn load_dynamic(&self, elf: &ElfFile, load_info: &mut LoadInfo) -> Result<(), LoadError> {
        for ph in &elf.program_headers {
            if ph.p_type == PT_DYNAMIC {
                load_info.dynamic_addr = ph.vaddr();
                load_info.dynamic_size = ph.mem_size();
                break;
            }
        }
        Ok(())
    }

    fn load_tls(&self, elf: &ElfFile, load_info: &mut LoadInfo) -> Result<(), LoadError> {
        for ph in &elf.program_headers {
            if ph.p_type == PT_TLS {
                load_info.tls_addr = ph.vaddr();
                load_info.tls_size = ph.mem_size();
                break;
            }
        }
        Ok(())
    }

    fn create_user_stack(&self, vmm: &mut VirtualMemoryManager, ttbr: usize) -> Result<usize, LoadError> {
        let stack_addr = USER_SPACE_END - USER_STACK_SIZE;

        for i in 0..(USER_STACK_SIZE / PAGE_SIZE) {
            let virt = stack_addr + i * PAGE_SIZE;
            let phys = self.allocate_physical_page();

            let attrs = 0x80 | 0x0 | 0x400 | 0x300;
            if !vmm.map_page(ttbr, virt, phys, attrs) {
                return Err(LoadError::MappingFailed);
            }
        }

        Ok(stack_addr)
    }

    fn create_user_heap(&self, vmm: &mut VirtualMemoryManager, ttbr: usize) -> Result<usize, LoadError> {
        let heap_addr = 0x0000_0040_0000_0000;

        for i in 0..(USER_HEAP_SIZE / PAGE_SIZE) {
            let virt = heap_addr + i * PAGE_SIZE;
            let phys = self.allocate_physical_page();

            let attrs = 0x80 | 0x0 | 0x400 | 0x300;
            if !vmm.map_page(ttbr, virt, phys, attrs) {
                return Err(LoadError::MappingFailed);
            }
        }

        Ok(heap_addr)
    }

    fn load_interp_executable(&mut self, interp_path: &str, vmm: &mut VirtualMemoryManager, ttbr: usize) -> Result<usize, LoadError> {
        let data = self.read_file(interp_path)?;
        let elf = ElfFile::parse(data).ok_or(LoadError::InvalidElf)?;

        if !elf.header.is_executable() && !elf.header.is_shared_object() {
            return Err(LoadError::InvalidElfType);
        }

        self.load_segments(&elf, &mut LoadInfo {
            entry_point: 0,
            stack_pointer: 0,
            program_break: 0,
            tls_addr: 0,
            tls_size: 0,
            interp_path: String::new(),
            dynamic_addr: 0,
            dynamic_size: 0,
        }, vmm, ttbr)?;

        Ok(elf.header.e_entry as usize)
    }

    fn read_file(&self, path: &str) -> Result<Vec<u8>, LoadError> {
        let data = crate::loader::read_file(path)
            .ok_or(LoadError::FileNotFound)?;
        Ok(data)
    }

    fn allocate_physical_page(&self) -> usize {
        use uefi::boot;

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
pub enum LoadError {
    InvalidElf,
    InvalidElfType,
    FileNotFound,
    AddressSpaceCreationFailed,
    MappingFailed,
    LibraryLoadFailed,
    OutOfMemory,
}
