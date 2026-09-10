use crate::color::Color;
use crate::screen::Screen;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::{Deref, DerefMut};
use core::ptr;

#[inline(always)]
fn blend_u32(bg: u32, fg: u32, a: u32) -> u32 {
    if a == 0 {
        return bg;
    }
    if a >= 255 {
        return fg;
    }
    let inv = 255 - a;
    let r = (((fg >> 16) & 0xFF) * a + ((bg >> 16) & 0xFF) * inv) / 255;
    let g = (((fg >> 8) & 0xFF) * a + ((bg >> 8) & 0xFF) * inv) / 255;
    let b = ((fg & 0xFF) * a + (bg & 0xFF) * inv) / 255;
    0xFF00_0000 | (r << 16) | (g << 8) | b
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn avx2_available() -> bool {
    use core::arch::x86_64::{__cpuid, __cpuid_count, _xgetbv};
    use core::sync::atomic::{AtomicU8, Ordering};

    // CPU/OS AVX state does not change while this UEFI image is running.
    static AVAILABLE: AtomicU8 = AtomicU8::new(0); // 0 unknown, 1 no, 2 yes
    match AVAILABLE.load(Ordering::Relaxed) {
        1 => return false,
        2 => return true,
        _ => {}
    }

    let available = unsafe {
        let leaf1 = __cpuid(1);
        const AVX: u32 = 1 << 28;
        const OSXSAVE: u32 = 1 << 27;
        if leaf1.ecx & (AVX | OSXSAVE) != (AVX | OSXSAVE) || (_xgetbv(0) & 0x6) != 0x6 {
            false
        } else {
            (__cpuid_count(7, 0).ebx & (1 << 5)) != 0
        }
    };
    AVAILABLE.store(if available { 2 } else { 1 }, Ordering::Relaxed);
    available
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_alpha_avx2(src: *const u32, dst: *mut u32, len: usize) {
    use core::arch::x86_64::*;

    let zero = _mm256_setzero_si256();
    let full_alpha = _mm256_set1_epi32(255);
    let mut px = 0usize;

    while px + 8 <= len {
        let sp = _mm256_loadu_si256(src.add(px) as *const __m256i);
        let a = _mm256_srli_epi32(sp, 24);
        let zero_alpha = _mm256_cmpeq_epi32(a, zero);
        if _mm256_movemask_epi8(zero_alpha) == -1 {
            px += 8;
            continue;
        }

        let opaque_alpha = _mm256_cmpeq_epi32(a, full_alpha);
        if _mm256_movemask_epi8(opaque_alpha) == -1 {
            _mm256_storeu_si256(dst.add(px) as *mut __m256i, sp);
            px += 8;
            continue;
        }

        // Mixed-alpha blocks are relatively rare in the compositor. Keep the
        // exact scalar blend here: LLVM's x86 UEFI legalizer crashes on the
        // AVX2 32-bit multiply sequence during fat LTO. Fully transparent and
        // fully opaque blocks still take the 8-pixel AVX2 fast paths above.
        for lane in 0..8 {
            let pixel = *src.add(px + lane);
            let alpha = pixel >> 24;
            if alpha != 0 {
                let old = *dst.add(px + lane);
                *dst.add(px + lane) = blend_u32(old, pixel, alpha);
            }
        }
        px += 8;
    }

    for i in px..len {
        let sp = *src.add(i);
        let a = sp >> 24;
        if a != 0 {
            let old = *dst.add(i);
            *dst.add(i) = blend_u32(old, sp, a);
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn blend_global_alpha_avx2(src: *const u32, dst: *mut u32, len: usize, alpha: u8) {
    use core::arch::x86_64::*;

    let zero = _mm256_setzero_si256();
    let a = _mm256_set1_epi16(alpha as i16);
    let inv = _mm256_set1_epi16((255 - alpha as u16) as i16);
    let one = _mm256_set1_epi16(1);
    let mut px = 0usize;
    while px + 8 <= len {
        let sp = _mm256_loadu_si256(src.add(px) as *const __m256i);
        let dp = _mm256_loadu_si256(dst.add(px) as *const __m256i);
        let sl = _mm256_unpacklo_epi8(sp, zero);
        let sh = _mm256_unpackhi_epi8(sp, zero);
        let dl = _mm256_unpacklo_epi8(dp, zero);
        let dh = _mm256_unpackhi_epi8(dp, zero);
        let sum_l = _mm256_add_epi16(_mm256_mullo_epi16(sl, a), _mm256_mullo_epi16(dl, inv));
        let sum_h = _mm256_add_epi16(_mm256_mullo_epi16(sh, a), _mm256_mullo_epi16(dh, inv));
        let div_l = _mm256_srli_epi16(
            _mm256_add_epi16(_mm256_add_epi16(sum_l, one), _mm256_srli_epi16(sum_l, 8)),
            8,
        );
        let div_h = _mm256_srli_epi16(
            _mm256_add_epi16(_mm256_add_epi16(sum_h, one), _mm256_srli_epi16(sum_h, 8)),
            8,
        );
        let packed = _mm256_packus_epi16(div_l, div_h);
        _mm256_storeu_si256(dst.add(px) as *mut __m256i, packed);
        px += 8;
    }
    for i in px..len {
        *dst.add(i) = blend_u32(*dst.add(i), *src.add(i), alpha as u32);
    }
}

enum LayerBuffer {
    Owned(Vec<u32>),
    Borrowed { ptr: *mut u32, len: usize },
}

impl Deref for LayerBuffer {
    type Target = [u32];

    fn deref(&self) -> &[u32] {
        match self {
            Self::Owned(buffer) => buffer,
            Self::Borrowed { ptr, len } => unsafe { core::slice::from_raw_parts(*ptr, *len) },
        }
    }
}

impl DerefMut for LayerBuffer {
    fn deref_mut(&mut self) -> &mut [u32] {
        match self {
            Self::Owned(buffer) => buffer,
            Self::Borrowed { ptr, len } => {
                unsafe { core::slice::from_raw_parts_mut(*ptr, *len) }
            }
        }
    }
}

pub struct LayerSystem {
    pub(crate) width: usize,
    pub(crate) height: usize,
    buf: LayerBuffer,
    frame_count: u64,
    clip_stack: Vec<(usize, usize, usize, usize)>,
    clip: Option<(usize, usize, usize, usize)>,
    dirty: bool,
    dirty_x0: usize,
    dirty_y0: usize,
    dirty_x1: usize,
    dirty_y1: usize,
}


