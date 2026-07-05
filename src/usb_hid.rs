// USB HID report descriptor parser.
//
// Parses the HID report descriptor to extract field layouts for
// absolute pointer (tablet) devices.

/// A single field parsed from the HID report descriptor.
#[derive(Clone, Copy, Debug, Default)]
pub struct HidField {
    pub usage_page: u16,
    pub usage: u16,
    pub logical_min: i32,
    pub logical_max: i32,
    pub report_size: u8,
    pub report_count: u8,
    pub is_constant: bool,
    pub is_variable: bool,
    pub is_relative: bool,
}

/// Parsed report layout for a mouse/tablet.
#[derive(Clone, Copy, Debug, Default)]
pub struct HidReportLayout {
    pub button_bits: u8,
    pub x_offset_bits: u16,
    pub x_size_bits: u8,
    pub x_max: i32,
    pub y_offset_bits: u16,
    pub y_size_bits: u8,
    pub y_max: i32,
    pub report_size_bytes: u16,
    pub is_absolute: bool,
}

/// Parse a HID report descriptor and produce a report layout.
pub fn parse_hid_report_desc(desc: &[u8]) -> HidReportLayout {
    let mut layout = HidReportLayout::default();
    let mut usage_page: u16 = 0;
    let mut usage: u16 = 0;
    let mut _logical_min: i32 = 0;
    let mut logical_max: i32 = 0;
    let mut report_size: u8 = 0;
    let mut report_count: u8 = 0;
    let mut bit_offset: u16 = 0;
    let mut in_collection = false;

    let mut i = 0;
    while i < desc.len() {
        let b = desc[i];
        let b_size = b & 0x03;
        let b_type = (b >> 2) & 0x03;
        let b_tag = (b >> 4) & 0x0F;

        let mut value: i32 = 0;
        match b_size {
            0 => value = 0,
            1 => { if i + 1 < desc.len() { value = desc[i + 1] as i8 as i32; } }
            2 => { if i + 2 < desc.len() { value = i16::from_le_bytes([desc[i+1], desc[i+2]]) as i32; } }
            _ => { i += 1 + desc.get(i + 1).copied().unwrap_or(0) as usize + 1; continue; }
        }

        match b_type {
            0 => {
                // Main item
                match b_tag {
                    0x8 => { // Input
                        if in_collection && report_count > 0 {
                            let is_const = (value & 1) != 0;
                            let _is_var = (value & 2) != 0;
                            let is_rel = (value & 4) != 0;

                            if !is_const {
                                if usage_page == 0x09 && usage >= 1 && usage <= 5 {
                                    layout.button_bits += report_count as u8;
                                }
                                if usage_page == 0x01 {
                                    match usage {
                                        0x30 => {
                                            layout.x_offset_bits = bit_offset;
                                            layout.x_size_bits = report_size;
                                            layout.x_max = logical_max;
                                            layout.is_absolute = !is_rel;
                                        }
                                        0x31 => {
                                            layout.y_offset_bits = bit_offset;
                                            layout.y_size_bits = report_size;
                                            layout.y_max = logical_max;
                                            layout.is_absolute = !is_rel;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            bit_offset += report_size as u16 * report_count as u16;
                        }
                    }
                    0x9 => { in_collection = true; }
                    0xA => { in_collection = false; }
                    _ => {}
                }
            }
            1 => {
                // Global item
                match b_tag {
                    0x0 => usage_page = value as u16,
                    0x1 => _logical_min = value,
                    0x2 => logical_max = value,
                    0x7 => report_size = value as u8,
                    0x9 => report_count = value as u8,
                    _ => {}
                }
            }
            2 => {
                // Local item
                match b_tag {
                    0x0 => usage = value as u16,
                    _ => {}
                }
            }
            _ => {}
        }

        i += 1 + b_size as usize;
    }

    layout.report_size_bytes = ((bit_offset + 7) / 8) as u16;
    layout
}

/// A parsed mouse/tablet event.
#[derive(Clone, Copy, Debug, Default)]
pub struct HidParsedEvent {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub is_absolute: bool,
    pub x_max: i32,
    pub y_max: i32,
}

/// Extract mouse event from an input report using the parsed layout.
pub fn parse_input_report(layout: &HidReportLayout, report: &[u8]) -> HidParsedEvent {
    let mut ev = HidParsedEvent::default();

    // Buttons are at bit offset 0
    for i in 0..layout.button_bits.min(8) {
        let byte = (i / 8) as usize;
        let bit = i % 8;
        if byte < report.len() && report[byte] & (1 << bit) != 0 {
            ev.buttons |= 1 << i;
        }
    }

    ev.x = extract_field(report, layout.x_offset_bits, layout.x_size_bits);
    ev.y = extract_field(report, layout.y_offset_bits, layout.y_size_bits);
    ev.is_absolute = layout.is_absolute;
    ev.x_max = layout.x_max;
    ev.y_max = layout.y_max;

    ev
}

fn extract_field(report: &[u8], bit_offset: u16, bit_size: u8) -> i32 {
    let mut val: u32 = 0;
    for i in 0..bit_size as u16 {
        let pos = bit_offset + i;
        let byte = (pos / 8) as usize;
        let bit = pos % 8;
        if byte < report.len() {
            if report[byte] & (1 << bit) != 0 {
                val |= 1 << i;
            }
        }
    }
    val as i32
}
