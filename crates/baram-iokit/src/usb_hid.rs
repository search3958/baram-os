//! Small, allocation-friendly USB HID report descriptor parser.
//!
//! Pointing devices do not all use the three-byte boot-mouse report.  In
//! particular, trackpads normally use report IDs and bit-packed absolute X/Y
//! fields.  The parser below keeps one layout per input report so the mouse
//! driver can decode both kinds without guessing from the packet length.

extern crate alloc;

use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HidReportLayout {
    /// Zero means that the device does not use report IDs.
    pub report_id: u8,
    pub application_usage_page: u16,
    pub application_usage: u16,
    pub button_offset_bits: u16,
    pub button_bits: u8,
    pub tip_offset_bits: u16,
    pub has_tip: bool,
    pub x_offset_bits: u16,
    pub x_size_bits: u8,
    pub x_min: i32,
    pub x_max: i32,
    pub y_offset_bits: u16,
    pub y_size_bits: u8,
    pub y_min: i32,
    pub y_max: i32,
    pub wheel_offset_bits: u16,
    pub wheel_size_bits: u8,
    pub wheel_min: i32,
    /// Includes the leading report-ID byte when one is present.
    pub report_size_bytes: u16,
    pub is_absolute: bool,
}

impl HidReportLayout {
    pub fn is_pointing(&self) -> bool {
        self.x_size_bits != 0 && self.y_size_bits != 0
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct GlobalState {
    usage_page: u16,
    logical_min: i32,
    logical_max: i32,
    report_size: u8,
    report_count: u8,
    report_id: u8,
}

#[derive(Default)]
struct LocalState {
    usages: Vec<u16>,
    usage_min: Option<u16>,
    usage_max: Option<u16>,
}

impl LocalState {
    fn clear(&mut self) {
        self.usages.clear();
        self.usage_min = None;
        self.usage_max = None;
    }

    fn usage_at(&self, index: usize) -> Option<u16> {
        if let Some(usage) = self.usages.get(index) {
            return Some(*usage);
        }
        match (self.usage_min, self.usage_max) {
            (Some(min), Some(max)) => {
                let usage = min.saturating_add(index as u16);
                if usage <= max {
                    Some(usage)
                } else {
                    Some(max)
                }
            }
            _ => self.usages.last().copied(),
        }
    }
}

fn layout_index(
    layouts: &mut Vec<HidReportLayout>,
    report_id: u8,
    application: (u16, u16),
) -> usize {
    if let Some(index) = layouts
        .iter()
        .position(|layout| layout.report_id == report_id)
    {
        index
    } else {
        layouts.push(HidReportLayout {
            report_id,
            application_usage_page: application.0,
            application_usage: application.1,
            ..HidReportLayout::default()
        });
        layouts.len() - 1
    }
}

fn item_unsigned(bytes: &[u8]) -> u32 {
    bytes.iter().enumerate().fold(0u32, |value, (index, byte)| {
        value | ((*byte as u32) << (index * 8))
    })
}

fn signed_value(value: u32, byte_count: usize) -> i32 {
    if byte_count == 0 {
        return 0;
    }
    let bits = (byte_count * 8) as u32;
    if bits >= 32 {
        value as i32
    } else {
        ((value << (32 - bits)) as i32) >> (32 - bits)
    }
}

/// Parse every pointing input report in a HID report descriptor.
pub fn parse_hid_report_descs(desc: &[u8]) -> Vec<HidReportLayout> {
    let mut layouts = Vec::new();
    let mut bit_offsets = [0u16; 256];
    let mut global = GlobalState::default();
    let mut global_stack: Vec<GlobalState> = Vec::new();
    let mut local = LocalState::default();
    let mut application = (0u16, 0u16);
    let mut collection_apps: Vec<(u16, u16)> = Vec::new();
    let mut i = 0usize;

    while i < desc.len() {
        let prefix = desc[i];
        if prefix == 0xfe {
            // Long item: 0xfe, data length, long tag, data...
            if i + 2 >= desc.len() {
                break;
            }
            i = (i + 3 + desc[i + 1] as usize).min(desc.len());
            continue;
        }

        let encoded_size = (prefix & 0x03) as usize;
        let size = if encoded_size == 3 { 4 } else { encoded_size };
        if i + 1 + size > desc.len() {
            break;
        }
        let data = &desc[i + 1..i + 1 + size];
        let unsigned = item_unsigned(data);
        let signed = signed_value(unsigned, size);
        let item_type = (prefix >> 2) & 0x03;
        let tag = (prefix >> 4) & 0x0f;

        match item_type {
            0 => {
                // Main items clear the local usage state after being consumed.
                if tag == 0x8 {
                    let flags = unsigned;
                    let is_constant = flags & 0x01 != 0;
                    let is_variable = flags & 0x02 != 0;
                    let is_relative = flags & 0x04 != 0;
                    let report_id = global.report_id;
                    let base = bit_offsets[report_id as usize];
                    let count = global.report_count as usize;
                    let field_size = global.report_size as u16;

                    if !is_constant && is_variable && count != 0 && field_size != 0 {
                        let index = layout_index(&mut layouts, report_id, application);
                        let layout = &mut layouts[index];
                        for field in 0..count {
                            let Some(usage) = local.usage_at(field) else {
                                continue;
                            };
                            let offset =
                                base.saturating_add(field_size.saturating_mul(field as u16));
                            match (global.usage_page, usage) {
                                (0x09, 1..=8) => {
                                    if layout.button_bits == 0 {
                                        layout.button_offset_bits = offset;
                                    }
                                    layout.button_bits =
                                        layout.button_bits.saturating_add(1).min(8);
                                }
                                (0x0d, 0x42) => {
                                    layout.tip_offset_bits = offset;
                                    layout.has_tip = true;
                                }
                                (0x01, 0x30) if layout.x_size_bits == 0 => {
                                    layout.x_offset_bits = offset;
                                    layout.x_size_bits = global.report_size;
                                    layout.x_min = global.logical_min;
                                    layout.x_max = global.logical_max;
                                    layout.is_absolute = !is_relative;
                                }
                                (0x01, 0x31) if layout.y_size_bits == 0 => {
                                    layout.y_offset_bits = offset;
                                    layout.y_size_bits = global.report_size;
                                    layout.y_min = global.logical_min;
                                    layout.y_max = global.logical_max;
                                    layout.is_absolute = !is_relative;
                                }
                                (0x01, 0x38) if layout.wheel_size_bits == 0 => {
                                    layout.wheel_offset_bits = offset;
                                    layout.wheel_size_bits = global.report_size;
                                    layout.wheel_min = global.logical_min;
                                }
                                _ => {}
                            }
                        }
                    }

                    bit_offsets[report_id as usize] =
                        base.saturating_add(field_size.saturating_mul(global.report_count as u16));
                } else if tag == 0xa {
                    collection_apps.push(application);
                    if unsigned & 0xff == 1 {
                        application = (global.usage_page, local.usage_at(0).unwrap_or(0));
                    }
                } else if tag == 0xc {
                    application = collection_apps.pop().unwrap_or((0, 0));
                }
                local.clear();
            }
            1 => match tag {
                0x0 => global.usage_page = unsigned as u16,
                0x1 => global.logical_min = signed,
                0x2 => {
                    // HID logical maxima are unsigned when the minimum is non-negative.
                    global.logical_max = if global.logical_min < 0 {
                        signed
                    } else {
                        unsigned as i32
                    };
                }
                0x7 => global.report_size = unsigned as u8,
                0x8 => global.report_id = unsigned as u8,
                0x9 => global.report_count = unsigned as u8,
                0xa => global_stack.push(global),
                0xb => {
                    if let Some(saved) = global_stack.pop() {
                        global = saved;
                    }
                }
                _ => {}
            },
            2 => match tag {
                0x0 => local.usages.push(unsigned as u16),
                0x1 => local.usage_min = Some(unsigned as u16),
                0x2 => local.usage_max = Some(unsigned as u16),
                _ => {}
            },
            _ => {}
        }

        i += 1 + size;
    }

    layouts.retain(HidReportLayout::is_pointing);
    for layout in &mut layouts {
        let bits = bit_offsets[layout.report_id as usize];
        layout.report_size_bytes =
            (bits.saturating_add(7) / 8).saturating_add((layout.report_id != 0) as u16);
    }
    layouts
}

/// Compatibility helper for callers that only need the first pointing report.
pub fn parse_hid_report_desc(desc: &[u8]) -> HidReportLayout {
    parse_hid_report_descs(desc)
        .into_iter()
        .next()
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HidParsedEvent {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub touching: bool,
    pub wheel: i32,
    pub is_absolute: bool,
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
}

fn extract_unsigned(report: &[u8], bit_offset: u16, bit_size: u8) -> u32 {
    let mut value = 0u32;
    for bit_index in 0..bit_size.min(32) as u16 {
        let position = bit_offset.saturating_add(bit_index);
        let byte = (position / 8) as usize;
        let bit = position % 8;
        if report.get(byte).copied().unwrap_or(0) & (1 << bit) != 0 {
            value |= 1 << bit_index;
        }
    }
    value
}

fn extract_value(report: &[u8], bit_offset: u16, bit_size: u8, logical_min: i32) -> i32 {
    let value = extract_unsigned(report, bit_offset, bit_size);
    if logical_min < 0 && bit_size != 0 && bit_size < 32 {
        ((value << (32 - bit_size)) as i32) >> (32 - bit_size)
    } else {
        value as i32
    }
}

pub fn parse_input_report(layout: &HidReportLayout, report: &[u8]) -> Option<HidParsedEvent> {
    let payload = if layout.report_id != 0 {
        if report.first().copied() != Some(layout.report_id) {
            return None;
        }
        &report[1..]
    } else {
        report
    };
    if report.len() < layout.report_size_bytes as usize || !layout.is_pointing() {
        return None;
    }

    let mut event = HidParsedEvent {
        x: extract_value(
            payload,
            layout.x_offset_bits,
            layout.x_size_bits,
            layout.x_min,
        ),
        y: extract_value(
            payload,
            layout.y_offset_bits,
            layout.y_size_bits,
            layout.y_min,
        ),
        wheel: extract_value(
            payload,
            layout.wheel_offset_bits,
            layout.wheel_size_bits,
            layout.wheel_min,
        ),
        is_absolute: layout.is_absolute,
        x_min: layout.x_min,
        x_max: layout.x_max,
        y_min: layout.y_min,
        y_max: layout.y_max,
        ..HidParsedEvent::default()
    };

    event.touching = layout.has_tip && extract_unsigned(payload, layout.tip_offset_bits, 1) != 0;

    for index in 0..layout.button_bits.min(8) {
        if extract_unsigned(payload, layout.button_offset_bits + index as u16, 1) != 0 {
            event.buttons |= 1 << index;
        }
    }
    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_boot_mouse_descriptor_and_signed_motion() {
        let descriptor = [
            0x05, 0x01, 0x09, 0x02, 0xa1, 0x01, 0x09, 0x01, 0xa1, 0x00, 0x05, 0x09, 0x19, 0x01,
            0x29, 0x03, 0x15, 0x00, 0x25, 0x01, 0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01,
            0x75, 0x05, 0x81, 0x03, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x38, 0x15, 0x81,
            0x25, 0x7f, 0x75, 0x08, 0x95, 0x03, 0x81, 0x06, 0xc0, 0xc0,
        ];
        let layouts = parse_hid_report_descs(&descriptor);
        assert_eq!(layouts.len(), 1);
        let event = parse_input_report(&layouts[0], &[0x01, 0xfb, 0x07, 0xff]).unwrap();
        assert_eq!(
            (event.buttons, event.x, event.y, event.wheel),
            (1, -5, 7, -1)
        );
        assert!(!event.is_absolute);
    }

    #[test]
    fn parses_report_id_and_absolute_trackpad_coordinates() {
        let descriptor = [
            0x05, 0x0d, 0x09, 0x05, 0xa1, 0x01, 0x85, 0x07, 0x09, 0x42, 0x15, 0x00, 0x25, 0x01,
            0x75, 0x01, 0x95, 0x01, 0x81, 0x02, 0x75, 0x07, 0x95, 0x01, 0x81, 0x03, 0x05, 0x01,
            0x09, 0x30, 0x09, 0x31, 0x15, 0x00, 0x26, 0xff, 0x0f, 0x75, 0x10, 0x95, 0x02, 0x81,
            0x02, 0xc0,
        ];
        let layouts = parse_hid_report_descs(&descriptor);
        assert_eq!(layouts.len(), 1);
        let event = parse_input_report(&layouts[0], &[7, 1, 0x34, 0x02, 0x78, 0x01]).unwrap();
        assert_eq!(
            (event.x, event.y, event.x_max, event.buttons),
            (0x234, 0x178, 4095, 0)
        );
        assert!(event.touching);
        assert_eq!(
            (
                layouts[0].application_usage_page,
                layouts[0].application_usage
            ),
            (0x0d, 0x05)
        );
        assert!(event.is_absolute);
    }
}
