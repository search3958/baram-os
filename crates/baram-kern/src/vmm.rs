#![no_std]

extern crate alloc;

use alloc::vec::Vec;

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SHIFT: usize = 12;
pub const PAGE_TABLE_LEVELS: usize = 4;

pub const DESCRIPTOR_VALID: u64 = 1 << 0;
pub const DESCRIPTOR_TABLE: u64 = 1 << 1;
pub const DESCRIPTOR_BLOCK: u64 = 1 << 1;
pub const DESCRIPTOR_PAGE: u64 = 0 << 1;

pub const ATTRIB_NORMAL: u64 = 0 << 2;
pub const ATTRIB_DEVICE: u64 = 1 << 2;
pub const ATTRIB_MEMORY: u64 = 0 << 2;

pub const ATTRIB_XN: u64 = 1 << 54;
pub const ATTRIB_PXN: u64 = 1 << 53;
pub const ATTRIB_CONTIGUOUS: u64 = 1 << 52;

pub const ATTRIB_AP_RW: u64 = 0 << 6;
pub const ATTRIB_AP_RO: u64 = 1 << 6;
pub const ATTRIB_AP_EL1: u64 = 0 << 6;
pub const ATTRIB_AP_EL0: u64 = 1 << 6;

pub const ATTRIB_AF: u64 = 1 << 10;
pub const ATTRIB_SH_INNER: u64 = 3 << 8;
pub const ATTRIB_SH_OUTER: u64 = 2 << 8;
pub const ATTRIB_SH_NONE: u64 = 0 << 8;

pub const TCR_T0SZ: u64 = 16 << 0;
pub const TCR_T1SZ: u64 = 16 << 16;
pub const TCR_TG0_4K: u64 = 0 << 14;
pub const TCR_TG1_4K: u64 = 1 << 30;
pub const TCR_IPS_48BIT: u64 = 2 << 32;
pub const TCR_SH_INNER: u64 = 3 << 28;
pub const TCR_SH_OUTER: u64 = 2 << 28;
pub const TCR_IRGN_WB: u64 = 1 << 8;
pub const TCR_ORGN_WB: u64 = 1 << 10;

pub const MAIR_NORMAL: u64 = 0xFF;
pub const MAIR_DEVICE: u64 = 0x04;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PageTableEntry {
    pub value: u64,
}

impl PageTableEntry {
    pub const fn new() -> Self {
        Self { value: 0 }
    }

    pub const fn valid() -> Self {
        Self { value: DESCRIPTOR_VALID }
    }

    pub const fn table() -> Self {
        Self { value: DESCRIPTOR_VALID | DESCRIPTOR_TABLE }
    }

    pub const fn block() -> Self {
        Self { value: DESCRIPTOR_VALID | DESCRIPTOR_BLOCK }
    }

    pub const fn page() -> Self {
        Self { value: DESCRIPTOR_VALID | DESCRIPTOR_PAGE }
    }

    pub fn is_valid(&self) -> bool {
        self.value & DESCRIPTOR_VALID != 0
    }

    pub fn is_table(&self) -> bool {
        self.value & DESCRIPTOR_TABLE != 0
    }

    pub fn is_block(&self) -> bool {
        self.value & DESCRIPTOR_BLOCK != 0 && !self.is_table()
    }

    pub fn is_page(&self) -> bool {
        self.value & DESCRIPTOR_PAGE != 0 && !self.is_table()
    }

    pub fn next_table_addr(&self) -> usize {
        (self.value & 0x0000_FFFF_FFFF_F000) as usize
    }

    pub fn set_next_table(&mut self, addr: usize) {
        self.value = (self.value & !0x0000_FFFF_FFFF_F000) | (addr as u64 & 0x0000_FFFF_FFFF_F000);
    }

    pub fn output_addr(&self) -> usize {
        (self.value & 0x0000_FFFF_FFFF_F000) as usize
    }

    pub fn set_output_addr(&mut self, addr: usize) {
        self.value = (self.value & !0x0000_FFFF_FFFF_F000) | (addr as u64 & 0x0000_FFFF_FFFF_F000);
    }

    pub fn set_attributes(&mut self, attrs: u64) {
        self.value = (self.value & 0x0000_FFFF_FFFF_F000) | (attrs & 0x0000_FFFF_FFFF_0FFF);
    }

    pub fn get_attributes(&self) -> u64 {
        self.value & 0x0000_0000_0000_0FFF
    }
}

pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::new(); 512],
        }
    }

    pub fn get_entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }

    pub fn get_entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }

    pub fn set_entry(&mut self, index: usize, entry: PageTableEntry) {
        self.entries[index] = entry;
    }
}

pub struct VirtualMemoryManager {
    page_tables: Vec<*mut PageTable>,
    kernel_ttbr0: usize,
    kernel_ttbr1: usize,
    current_asid: u8,
    next_asid: u8,
}

impl VirtualMemoryManager {
    pub fn new() -> Self {
        Self {
            page_tables: Vec::new(),
            kernel_ttbr0: 0,
            kernel_ttbr1: 0,
            current_asid: 0,
            next_asid: 1,
        }
    }

    pub fn init(&mut self, kernel_ttbr0: usize, kernel_ttbr1: usize) {
        self.kernel_ttbr0 = kernel_ttbr0;
        self.kernel_ttbr1 = kernel_ttbr1;
    }

    pub fn create_address_space(&mut self) -> usize {
        let ttbr = self.alloc_page_table();
        if ttbr == 0 {
            return 0;
        }

        let kernel_table = unsafe { &*(self.kernel_ttbr0 as *const PageTable) };
        let new_table = unsafe { &mut *(ttbr as *mut PageTable) };

        for i in 256..512 {
            new_table.entries[i] = kernel_table.entries[i];
        }

        ttbr
    }

    pub fn map_page(&mut self, ttbr: usize, virt_addr: usize, phys_addr: usize, attrs: u64) -> bool {
        let l0_index = (virt_addr >> 39) & 0x1FF;
        let l1_index = (virt_addr >> 30) & 0x1FF;
        let l2_index = (virt_addr >> 21) & 0x1FF;
        let l3_index = (virt_addr >> 12) & 0x1FF;

        let l0 = unsafe { &mut *(ttbr as *mut PageTable) };
        let l1 = self.get_or_create_table(l0, l0_index);
        if l1 == 0 { return false; }

        let l1_table = unsafe { &mut *(l1 as *mut PageTable) };
        let l2 = self.get_or_create_table(l1_table, l1_index);
        if l2 == 0 { return false; }

        let l2_table = unsafe { &mut *(l2 as *mut PageTable) };
        let l3 = self.get_or_create_table(l2_table, l2_index);
        if l3 == 0 { return false; }

        let l3_table = unsafe { &mut *(l3 as *mut PageTable) };
        let mut entry = PageTableEntry::page();
        entry.set_output_addr(phys_addr);
        entry.set_attributes(attrs | DESCRIPTOR_VALID | ATTRIB_AF | ATTRIB_SH_INNER);
        l3_table.set_entry(l3_index, entry);

        self.flush_tlb_entry(virt_addr);

        true
    }

    pub fn map_block(&mut self, ttbr: usize, virt_addr: usize, phys_addr: usize, size: usize, attrs: u64) -> bool {
        let mut offset = 0;
        while offset < size {
            let remaining = size - offset;
            let aligned_remaining = remaining & !0x1FF_FFFF;

            if aligned_remaining >= PAGE_SIZE * 512 && ((virt_addr + offset) & 0x1FF_FFFF) == 0 && ((phys_addr + offset) & 0x1FF_FFFF) == 0 {
                let l0_index = ((virt_addr + offset) >> 39) & 0x1FF;
                let l1_index = ((virt_addr + offset) >> 30) & 0x1FF;
                let l2_index = ((virt_addr + offset) >> 21) & 0x1FF;

                let l0 = unsafe { &mut *(ttbr as *mut PageTable) };
                let l1 = self.get_or_create_table(l0, l0_index);
                if l1 == 0 { return false; }

                let l1_table = unsafe { &mut *(l1 as *mut PageTable) };
                let l2 = self.get_or_create_table(l1_table, l1_index);
                if l2 == 0 { return false; }

                let l2_table = unsafe { &mut *(l2 as *mut PageTable) };
                let mut entry = PageTableEntry::block();
                entry.set_output_addr(phys_addr + offset);
                entry.set_attributes(attrs | DESCRIPTOR_VALID | ATTRIB_AF | ATTRIB_SH_INNER);
                l2_table.set_entry(l2_index, entry);

                self.flush_tlb_range(virt_addr + offset, PAGE_SIZE * 512);
                offset += PAGE_SIZE * 512;
            } else {
                if !self.map_page(ttbr, virt_addr + offset, phys_addr + offset, attrs) {
                    return false;
                }
                offset += PAGE_SIZE;
            }
        }

        true
    }

    pub fn unmap_page(&mut self, ttbr: usize, virt_addr: usize) -> bool {
        let l0_index = (virt_addr >> 39) & 0x1FF;
        let l1_index = (virt_addr >> 30) & 0x1FF;
        let l2_index = (virt_addr >> 21) & 0x1FF;
        let l3_index = (virt_addr >> 12) & 0x1FF;

        let l0 = unsafe { &*(ttbr as *const PageTable) };
        let l1_entry = l0.get_entry(l0_index);
        if !l1_entry.is_valid() || !l1_entry.is_table() {
            return false;
        }

        let l1 = unsafe { &*(l1_entry.next_table_addr() as *const PageTable) };
        let l2_entry = l1.get_entry(l1_index);
        if !l2_entry.is_valid() || !l2_entry.is_table() {
            return false;
        }

        let l2 = unsafe { &*(l2_entry.next_table_addr() as *const PageTable) };
        let l3_entry = l2.get_entry(l2_index);
        if !l3_entry.is_valid() || !l3_entry.is_table() {
            return false;
        }

        let l3 = unsafe { &mut *(l3_entry.next_table_addr() as *mut PageTable) };
        l3.set_entry(l3_index, PageTableEntry::new());

        self.flush_tlb_entry(virt_addr);

        true
    }

    pub fn translate(&self, ttbr: usize, virt_addr: usize) -> Option<usize> {
        let l0_index = (virt_addr >> 39) & 0x1FF;
        let l1_index = (virt_addr >> 30) & 0x1FF;
        let l2_index = (virt_addr >> 21) & 0x1FF;
        let l3_index = (virt_addr >> 12) & 0x1FF;
        let page_offset = virt_addr & 0xFFF;

        let l0 = unsafe { &*(ttbr as *const PageTable) };
        let l1_entry = l0.get_entry(l0_index);
        if !l1_entry.is_valid() { return None; }

        if l1_entry.is_table() {
            let l1 = unsafe { &*(l1_entry.next_table_addr() as *const PageTable) };
            let l2_entry = l1.get_entry(l1_index);
            if !l2_entry.is_valid() { return None; }

            if l2_entry.is_block() {
                return Some(l2_entry.output_addr() + (virt_addr & 0x3FFF_FFFF));
            }

            let l2 = unsafe { &*(l2_entry.next_table_addr() as *const PageTable) };
            let l3_entry = l2.get_entry(l2_index);
            if !l3_entry.is_valid() { return None; }

            if l3_entry.is_block() {
                return Some(l3_entry.output_addr() + (virt_addr & 0x1F_FFFF));
            }

            let l3 = unsafe { &*(l3_entry.next_table_addr() as *const PageTable) };
            let entry = l3.get_entry(l3_index);
            if !entry.is_valid() { return None; }

            return Some(entry.output_addr() + page_offset);
        }

        None
    }

    fn get_or_create_table(&mut self, parent: &mut PageTable, index: usize) -> usize {
        let entry = parent.get_entry(index);
        if entry.is_valid() && entry.is_table() {
            return entry.next_table_addr();
        }

        let new_table = self.alloc_page_table();
        if new_table == 0 {
            return 0;
        }

        let mut new_entry = PageTableEntry::table();
        new_entry.set_next_table(new_table);
        parent.set_entry(index, new_entry);

        new_table
    }

    fn alloc_page_table(&mut self) -> usize {
        let layout = core::alloc::Layout::new::<PageTable>();
        unsafe {
            let ptr = alloc::alloc::alloc(layout);
            if ptr.is_null() {
                0
            } else {
                let table = &mut *(ptr as *mut PageTable);
                table.entries = [PageTableEntry::new(); 512];
                ptr as usize
            }
        }
    }

    fn flush_tlb_entry(&self, _addr: usize) {
        unsafe {
            core::arch::asm!("dsb sy; isb");
        }
    }

    fn flush_tlb_range(&self, _addr: usize, _size: usize) {
        unsafe {
            core::arch::asm!("dsb sy; isb");
        }
    }

    pub fn switch_address_space(&self, ttbr: usize, asid: u8) {
        unsafe {
            let tcr = self.get_tcr();
            core::arch::asm!(
                "msr ttbr0_el1, {ttbr}",
                "msr tcr_el1, {tcr}",
                "dsb sy; isb",
                "tlbi vmalle1is",
                "dsb sy; isb",
                ttbr = in(reg) ttbr | ((asid as usize) << 48),
                tcr = in(reg) tcr,
            );
        }
    }

    fn get_tcr(&self) -> u64 {
        TCR_T0SZ | TCR_T1SZ | TCR_TG0_4K | TCR_TG1_4K | TCR_IPS_48BIT | TCR_SH_INNER | TCR_IRGN_WB | TCR_ORGN_WB
    }

    pub fn init_mmu(&self) {
        unsafe {
            let mair = (MAIR_NORMAL << 0) | (MAIR_DEVICE << 8);
            core::arch::asm!(
                "msr mair_el1, {mair}",
                "msr tcr_el1, {tcr}",
                "msr ttbr0_el1, {ttbr0}",
                "msr ttbr1_el1, {ttbr1}",
                "dsb sy; isb",
                "mrs {sctlr}, sctlr_el1",
                "orr {sctlr}, {sctlr}, #{enable_bit}",
                "msr sctlr_el1, {sctlr}",
                "dsb sy; isb",
                mair = in(reg) mair,
                tcr = in(reg) self.get_tcr(),
                ttbr0 = in(reg) self.kernel_ttbr0,
                ttbr1 = in(reg) self.kernel_ttbr1,
                sctlr = out(reg) _,
                enable_bit = const (1 << 0 | 1 << 2),
            );
        }
    }
}
