pub(crate) struct UiMonotonicClock {
    origin: u64,
    last: u64,
    frequency_hz: u64,
}

impl UiMonotonicClock {
    pub(crate) fn new() -> Option<Self> {
        let frequency_hz = monotonic_counter_frequency()?;
        let origin = monotonic_counter();
        Some(Self {
            origin,
            last: origin,
            frequency_hz,
        })
    }

    #[inline]
    fn frame_delta_ms(&mut self) -> u64 {
        let now = monotonic_counter();
        let ticks = now.wrapping_sub(self.last);
        self.last = now;
        // Firmware and virtual CPUs occasionally expose an inaccurate TSC
        // ratio or jump the counter. Never let one sample consume the entire
        // animation, and always advance at least one timer quantum.
        let measured = ((ticks as u128 * 1_000) / self.frequency_hz as u128) as u64;
        measured.clamp(1, 16)
    }

    #[inline]
    pub(crate) fn elapsed_ns(&self) -> u64 {
        let ticks = monotonic_counter().wrapping_sub(self.origin);
        ((ticks as u128 * 1_000_000_000) / self.frequency_hz as u128) as u64
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn monotonic_counter() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

#[cfg(target_arch = "x86_64")]
fn monotonic_counter_frequency() -> Option<u64> {
    use core::arch::x86_64::{__cpuid, __cpuid_count};
    unsafe {
        let max_leaf = __cpuid(0).eax;
        if max_leaf >= 0x15 {
            let leaf = __cpuid_count(0x15, 0);
            if leaf.eax != 0 && leaf.ebx != 0 && leaf.ecx != 0 {
                return Some((leaf.ecx as u64).saturating_mul(leaf.ebx as u64) / leaf.eax as u64);
            }
        }
        if max_leaf >= 0x16 {
            let mhz = __cpuid(0x16).eax;
            if mhz != 0 {
                return Some(mhz as u64 * 1_000_000);
            }
        }
        // Some virtual firmware hides leaves 0x15/0x16. Calibrate once at
        // startup instead of falling back to coalesced timer-event counts.
        let start = core::arch::x86_64::_rdtsc();
        uefi::boot::stall(core::time::Duration::from_millis(10));
        let elapsed = core::arch::x86_64::_rdtsc().wrapping_sub(start);
        if elapsed != 0 {
            return Some(elapsed.saturating_mul(100));
        }
    }
    None
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn monotonic_counter() -> u64 {
    let value: u64;
    unsafe {
        core::arch::asm!("mrs {0}, cntvct_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

#[cfg(target_arch = "aarch64")]
fn monotonic_counter_frequency() -> Option<u64> {
    let value: u64;
    unsafe {
        core::arch::asm!("mrs {0}, cntfrq_el0", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    if value == 0 {
        None
    } else {
        Some(value)
    }
}
