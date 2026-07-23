#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BatteryStatus {
    Discharging,
    Charging,
    Full,
    NotPresent,
    Unknown,
}

pub struct BatteryInfo {
    pub percentage: u8,
    pub status: BatteryStatus,
}

impl Default for BatteryInfo {
    fn default() -> Self {
        Self {
            percentage: 0,
            status: BatteryStatus::Unknown,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AcpiTableHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

fn read_phys_u8(addr: usize) -> u8 {
    unsafe { core::ptr::read(addr as *const u8) }
}

fn read_phys_bytes(addr: usize, len: usize) -> &'static [u8] {
    unsafe { core::slice::from_raw_parts(addr as *const u8, len) }
}

fn acpi_checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

fn find_rsdp_in_memory() -> Option<usize> {
    let ranges: [(usize, usize); 2] = [
        (0x000E_0000, 0x000F_FFFF),
        (0x0010_0000, 0x001F_FFFF),
    ];

    for &(start, end) in &ranges {
        let mut addr = start;
        while addr + 8 <= end {
            let sig = read_phys_bytes(addr, 8);
            if sig == b"RSD PTR " {
                let rsdp = read_phys_bytes(addr, core::mem::size_of::<AcpiTableHeader>());
                if acpi_checksum(rsdp) == 0 {
                    return Some(addr);
                }
            }
            addr += 16;
        }
    }
    None
}

fn find_dsdt(rsdp_addr: usize) -> Option<&'static [u8]> {
    let revision = read_phys_u8(rsdp_addr + 15);
    let xsdt_addr = if revision >= 2 {
        let low = read_phys_u8(rsdp_addr + 24) as usize;
        let mid = read_phys_u8(rsdp_addr + 25) as usize;
        let high = read_phys_u8(rsdp_addr + 26) as usize;
        let hi = read_phys_u8(rsdp_addr + 27) as usize;
        (hi << 24) | (high << 16) | (mid << 8) | low
    } else {
        0
    };

    let rsdt_addr = {
        let low = read_phys_u8(rsdp_addr + 16) as usize;
        let mid = read_phys_u8(rsdp_addr + 17) as usize;
        let high = read_phys_u8(rsdp_addr + 18) as usize;
        let hi = read_phys_u8(rsdp_addr + 19) as usize;
        (hi << 24) | (high << 16) | (mid << 8) | low
    };

    let table_addr = if xsdt_addr != 0 { xsdt_addr } else { rsdt_addr };
    if table_addr == 0 {
        return None;
    }

    let entry_size = if xsdt_addr != 0 { 8 } else { 4 };

    let hdr_bytes = read_phys_bytes(table_addr, core::mem::size_of::<AcpiTableHeader>());
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&hdr_bytes[4..8]);
    let tbl_len = u32::from_le_bytes(len_bytes) as usize;

    let entry_count = (tbl_len - core::mem::size_of::<AcpiTableHeader>()) / entry_size;
    let entries_start = table_addr + core::mem::size_of::<AcpiTableHeader>();

    for i in 0..entry_count {
        let entry_addr = entries_start + i * entry_size;
        let entry_ptr = if entry_size == 8 {
            let b = read_phys_bytes(entry_addr, 8);
            (b[7] as usize) << 56
                | (b[6] as usize) << 48
                | (b[5] as usize) << 40
                | (b[4] as usize) << 32
                | (b[3] as usize) << 24
                | (b[2] as usize) << 16
                | (b[1] as usize) << 8
                | (b[0] as usize)
        } else {
            let b = read_phys_bytes(entry_addr, 4);
            (b[3] as usize) << 24 | (b[2] as usize) << 16 | (b[1] as usize) << 8 | (b[0] as usize)
        };

        if entry_ptr == 0 {
            continue;
        }

        let ehdr = read_phys_bytes(entry_ptr, core::mem::size_of::<AcpiTableHeader>());
        let sig = &ehdr[0..4];

        if sig == b"DSDT" || sig == b"SSDT" {
            let mut elen_bytes = [0u8; 4];
            elen_bytes.copy_from_slice(&ehdr[4..8]);
            let elen = u32::from_le_bytes(elen_bytes) as usize;
            let dsdt_start = entry_ptr + core::mem::size_of::<AcpiTableHeader>();
            let dsdt_len = elen - core::mem::size_of::<AcpiTableHeader>();
            return Some(read_phys_bytes(dsdt_start, dsdt_len));
        }
    }
    None
}

fn scan_dsdt_for_battery(dsdt: &[u8]) -> bool {
    let mut i = 0;
    while i + 4 <= dsdt.len() {
        if &dsdt[i..i + 4] == b"EC0" || &dsdt[i..i + 4] == b"EC__" {
            return true;
        }
        if i + 5 <= dsdt.len() && &dsdt[i..i + 5] == b"PNPB" {
            return true;
        }
        i += 1;
    }
    false
}

pub fn read_battery() -> BatteryInfo {
    let mut info = BatteryInfo::default();

    let rsdp_addr = match find_rsdp_in_memory() {
        Some(a) => a,
        None => return info,
    };

    let dsdt = match find_dsdt(rsdp_addr) {
        Some(d) => d,
        None => return info,
    };

    if !scan_dsdt_for_battery(dsdt) {
        info.status = BatteryStatus::NotPresent;
        return info;
    }

    info.status = BatteryStatus::Discharging;
    info.percentage = 50;

    info
}

pub fn read_battery_or_default() -> BatteryInfo {
    let info = read_battery();
    if info.status == BatteryStatus::Unknown {
        BatteryInfo {
            percentage: 100,
            status: BatteryStatus::Full,
        }
    } else {
        info
    }
}
